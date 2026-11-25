//! The derive macros of `sdrkit-core`. Use them through `sdrkit_core::interface`.

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, GenericArgument, GenericParam, Lifetime,
    Lit, LitStr, Meta, Path, PathArguments, Type,
};

/// Implements `sdrkit_core::interface::Fields` and `ReadFields` for a struct with named fields, and makes one `Column`
/// constant for each struct field (the name of the field in upper case). `sdrkit_core::interface::Fields` lists the
/// attributes.
#[proc_macro_derive(Fields, attributes(field, fields))]
pub fn derive_fields(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match fields_impl(&input) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Implements `sdrkit_core::interface::Arguments` and `ReadArguments` for a struct with named fields, and makes one
/// constant for each struct field: an `Argument` for a required argument, a `Setting` for an `Option`.
/// `sdrkit_core::interface::Arguments` lists the attributes.
#[proc_macro_derive(Arguments, attributes(arg))]
pub fn derive_arguments(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match arguments_impl(&input) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// The text of the doc comment in `attrs`, its lines joined by a space.
fn doc_of(attrs: &[Attribute]) -> String {
    let mut out = String::new();
    for a in attrs {
        let Meta::NameValue(nv) = &a.meta else {
            continue;
        };
        if !nv.path.is_ident("doc") {
            continue;
        }
        if let Expr::Lit(l) = &nv.value {
            if let Lit::Str(s) = &l.lit {
                let line = s.value();
                let line = line.trim();
                if !out.is_empty() && !line.is_empty() {
                    out.push(' ');
                }
                out.push_str(line);
            }
        }
    }
    out
}

/// `T` if `ty` is `Option<T>`.
fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else {
        return None;
    };
    let last = p.path.segments.last()?;
    if last.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    match args.args.first()? {
        GenericArgument::Type(t) if args.args.len() == 1 => Some(t),
        _ => None,
    }
}

/// `tokens` with every lifetime replaced by `'static`.
fn with_static_lifetimes(tokens: TokenStream) -> TokenStream {
    let mut out = Vec::new();
    let mut after_tick = false;
    for t in tokens {
        match t {
            TokenTree::Group(g) => {
                let mut ng =
                    proc_macro2::Group::new(g.delimiter(), with_static_lifetimes(g.stream()));
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                after_tick = false;
            }
            TokenTree::Punct(p) => {
                after_tick = p.as_char() == '\'';
                out.push(TokenTree::Punct(p));
            }
            TokenTree::Ident(i) if after_tick => {
                out.push(TokenTree::Ident(Ident::new("static", i.span())));
                after_tick = false;
            }
            other => {
                out.push(other);
                after_tick = false;
            }
        }
    }
    out.into_iter().collect()
}

#[derive(Default)]
struct FieldAttr {
    label: Option<LitStr>,
    unit: Option<LitStr>,
    refers_to: Option<LitStr>,
    content: Option<Path>,
    shared: Option<Path>,
    role: Option<Ident>,
    reading: Option<syn::LitInt>,
}

fn field_attr(attrs: &[Attribute]) -> syn::Result<FieldAttr> {
    let mut out = FieldAttr::default();
    for a in attrs.iter().filter(|a| a.path().is_ident("field")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("label") {
                out.label = Some(m.value()?.parse()?);
            } else if m.path.is_ident("unit") {
                out.unit = Some(m.value()?.parse()?);
            } else if m.path.is_ident("refers_to") {
                out.refers_to = Some(m.value()?.parse()?);
            } else if m.path.is_ident("content") {
                out.content = Some(m.value()?.parse()?);
            } else if m.path.is_ident("shared") {
                out.shared = Some(m.value()?.parse()?);
            } else if m.path.is_ident("role") {
                out.role = Some(m.value()?.parse()?);
            } else if m.path.is_ident("reading") {
                out.reading = Some(m.value()?.parse()?);
            } else {
                return Err(
                    m.error("expected label, unit, refers_to, content, shared, role or reading")
                );
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// The type in `#[fields(part_of = Type)]`, if the struct has the attribute.
fn part_of(attrs: &[Attribute]) -> syn::Result<Option<Type>> {
    let mut out = None;
    for a in attrs.iter().filter(|a| a.path().is_ident("fields")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("part_of") {
                out = Some(m.value()?.parse()?);
                Ok(())
            } else {
                Err(m.error("expected part_of"))
            }
        })?;
    }
    Ok(out)
}

fn fields_impl(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let whole = part_of(&input.attrs)?;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "Fields is for a struct"));
    };
    let syn::Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "Fields is for a struct with named fields",
        ));
    };
    let mut lifetimes = Vec::new();
    for p in &input.generics.params {
        match p {
            GenericParam::Lifetime(l) => lifetimes.push(l.lifetime.clone()),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "a shape has lifetime parameters only",
                ))
            }
        }
    }
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let statics = lifetimes.iter().map(|_| quote!('static));
    let static_ty = if lifetimes.is_empty() {
        quote!(#name)
    } else {
        quote!(#name<#(#statics),*>)
    };
    // The read borrows for the first lifetime of the struct, or for any lifetime if the struct has none.
    let (read_impl, read_life) = match lifetimes.first() {
        Some(l) => (quote!(#impl_generics), l.clone()),
        None => {
            let l = Lifetime::new("'__read", Span::call_site());
            (quote!(<#l>), l)
        }
    };

    let core = quote!(::sdrkit_core);
    let mut consts = Vec::new();
    let mut list = Vec::new();
    let mut notes = Vec::new();
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    for f in &named.named {
        let ident = f.ident.as_ref().expect("a named field");
        let key = ident.to_string();
        let key = key.strip_prefix("r#").unwrap_or(&key).to_owned();
        let cname = format_ident!("{}", key.to_uppercase());
        let attr = field_attr(&f.attrs)?;
        let doc = doc_of(&f.attrs);
        let inner = option_inner(&f.ty);
        let optional = inner.is_some();
        let cell_ty = inner.unwrap_or(&f.ty);
        let cell_static = with_static_lifetimes(quote!(#cell_ty));
        let kind = quote!(<#cell_static as #core::interface::Cell<'static>>::Kind);
        if let Some(whole) = &whole {
            if attr.label.is_some()
                || attr.shared.is_some()
                || attr.unit.is_some()
                || attr.refers_to.is_some()
                || attr.reading.is_some()
            {
                return Err(syn::Error::new_spanned(
                    ident,
                    "a field of a part takes its declaration from the whole shape",
                ));
            }
            let column = quote!(<#whole>::#cname);
            // The part has the kind and the optionality of the whole shape.
            consts.push(quote! {
                const _: () = {
                    let _: &#core::interface::Column<#kind> = &#column;
                    assert!(#column.optional == #optional, "the field is optional in one of the two structs only");
                };
            });
            let field = quote!(#core::interface::Column::__field(&#column));
            list.push(quote!(&#column as &dyn #core::Declare));
            notes.push(quote!(#core::interface::FieldNote { optional: #optional, doc: #doc }));
            if optional {
                writes.push(quote! {
                    out.opt(#field, self.#ident.as_ref().map(#core::interface::Cell::give));
                });
                reads.push(quote! {
                    #ident: rec.get(#field).map(#core::interface::Cell::take)
                });
            } else {
                writes.push(quote! {
                    out.set(#field, #core::interface::Cell::give(&self.#ident));
                });
                reads.push(quote! {
                    #ident: #core::interface::Cell::take(rec.get(#field)?)
                });
            }
            continue;
        }
        let role = attr.role.iter();
        let reading = attr.reading.iter();
        let value = match (&attr.shared, &attr.label) {
            (Some(path), None) => {
                if attr.unit.is_some() || attr.refers_to.is_some() {
                    return Err(syn::Error::new_spanned(
                        path,
                        "a shared field takes its unit and its table from the constant",
                    ));
                }
                quote!({
                    assert!(
                        #core::interface::same_text(#path.key, #key),
                        "the key of the shared field is not the name of the struct field"
                    );
                    #path #(.role(#core::Role::#role))* #(.reading(#reading))*
                })
            }
            (None, Some(label)) => {
                let unit = attr
                    .unit
                    .clone()
                    .unwrap_or_else(|| LitStr::new("", Span::call_site()));
                let refers = attr.refers_to.iter();
                let content = attr.content.iter();
                quote!(#core::Field::new(#key, #label, #unit) #(.refers_to_table(#refers))* #(.content(#content))* #(.role(#core::Role::#role))* #(.reading(#reading))*)
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "give #[field(label = \"...\")] or #[field(shared = PATH)]",
                ))
            }
        };
        consts.push(quote! {
            #[doc = #doc]
            pub const #cname: #core::interface::Column<#kind> =
                #core::interface::Column::__new(#value, #optional);
        });
        let field = quote!(#core::interface::Column::__field(&<#static_ty>::#cname));
        list.push(quote!(&<#static_ty>::#cname as &dyn #core::Declare));
        notes.push(quote!(#core::interface::FieldNote { optional: #optional, doc: #doc }));
        if optional {
            writes.push(quote! {
                out.opt(#field, self.#ident.as_ref().map(#core::interface::Cell::give));
            });
            reads.push(quote! {
                #ident: rec.get(#field).map(#core::interface::Cell::take)
            });
        } else {
            writes.push(quote! {
                out.set(#field, #core::interface::Cell::give(&self.#ident));
            });
            reads.push(quote! {
                #ident: #core::interface::Cell::take(rec.get(#field)?)
            });
        }
    }
    let shape_name = name.to_string();
    let shape_doc = doc_of(&input.attrs);
    let consts = if whole.is_some() {
        quote!(#(#consts)*)
    } else {
        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #(#consts)*
            }
        }
    };
    Ok(quote! {
        #consts
        impl #impl_generics #core::interface::Fields for #name #ty_generics #where_clause {
            type Static = #static_ty;
            const SHAPE: #core::interface::Shape = #core::interface::Shape {
                name: #shape_name,
                doc: #shape_doc,
                fields: &[#(#list),*],
                notes: &[#(#notes),*],
            };
            fn write(&self, out: &mut impl #core::interface::Sink) {
                #(#writes)*
            }
        }
        impl #read_impl #core::interface::ReadFields<#read_life> for #name #ty_generics #where_clause {
            fn read(rec: &#read_life #core::Record) -> ::core::option::Option<Self> {
                ::core::option::Option::Some(Self { #(#reads),* })
            }
        }
    })
}

#[derive(Default)]
struct ArgAttr {
    label: Option<LitStr>,
    unit: Option<LitStr>,
    help: Option<LitStr>,
    refers_to: Option<LitStr>,
    refers_column: Option<LitStr>,
    shared: Option<Path>,
    default: Option<Expr>,
    min: Option<Expr>,
    max: Option<Expr>,
    step: Option<Expr>,
}

fn arg_attr(attrs: &[Attribute]) -> syn::Result<ArgAttr> {
    let mut out = ArgAttr::default();
    for a in attrs.iter().filter(|a| a.path().is_ident("arg")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("label") {
                out.label = Some(m.value()?.parse()?);
            } else if m.path.is_ident("unit") {
                out.unit = Some(m.value()?.parse()?);
            } else if m.path.is_ident("help") {
                out.help = Some(m.value()?.parse()?);
            } else if m.path.is_ident("refers_to") {
                out.refers_to = Some(m.value()?.parse()?);
            } else if m.path.is_ident("column") {
                out.refers_column = Some(m.value()?.parse()?);
            } else if m.path.is_ident("shared") {
                out.shared = Some(m.value()?.parse()?);
            } else if m.path.is_ident("default") {
                out.default = Some(m.value()?.parse()?);
            } else if m.path.is_ident("min") {
                out.min = Some(m.value()?.parse()?);
            } else if m.path.is_ident("max") {
                out.max = Some(m.value()?.parse()?);
            } else if m.path.is_ident("step") {
                out.step = Some(m.value()?.parse()?);
            } else {
                return Err(m.error(
                    "expected label, unit, help, refers_to, column, shared, default, min, max or step",
                ));
            }
            Ok(())
        })?;
    }
    Ok(out)
}

fn arguments_impl(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "Arguments is for a struct"));
    };
    let syn::Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "Arguments is for a struct with named fields",
        ));
    };
    let mut lifetimes = Vec::new();
    for p in &input.generics.params {
        match p {
            GenericParam::Lifetime(l) => lifetimes.push(l.lifetime.clone()),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "an arguments struct has lifetime parameters only",
                ))
            }
        }
    }
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let statics = lifetimes.iter().map(|_| quote!('static));
    let static_ty = if lifetimes.is_empty() {
        quote!(#name)
    } else {
        quote!(#name<#(#statics),*>)
    };
    let (read_impl, read_life) = match lifetimes.first() {
        Some(l) => (quote!(#impl_generics), l.clone()),
        None => {
            let l = Lifetime::new("'__read", Span::call_site());
            (quote!(<#l>), l)
        }
    };

    let core = quote!(::sdrkit_core);
    let mut consts = Vec::new();
    let mut list = Vec::new();
    let mut docs = Vec::new();
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    for f in &named.named {
        let ident = f.ident.as_ref().expect("a named field");
        let key = ident.to_string();
        let key = key.strip_prefix("r#").unwrap_or(&key).to_owned();
        let cname = format_ident!("{}", key.to_uppercase());
        let attr = arg_attr(&f.attrs)?;
        let doc = doc_of(&f.attrs);
        let inner = option_inner(&f.ty);
        let optional = inner.is_some();
        let ty = inner.unwrap_or(&f.ty);
        let cell_static = with_static_lifetimes(quote!(#ty));
        let kind = quote!(<#cell_static as #core::interface::Cell<'static>>::Kind);
        let limits = [&attr.min, &attr.max, &attr.step];
        let has_limits = limits.iter().any(|l| l.is_some());
        if !optional && (attr.default.is_some() || has_limits) {
            return Err(syn::Error::new_spanned(
                ident,
                "a required argument has no default and no limits: make the struct field an Option",
            ));
        }
        if has_limits && limits.iter().any(|l| l.is_none()) {
            return Err(syn::Error::new_spanned(
                ident,
                "give min, max and step together",
            ));
        }
        let value = match (&attr.shared, &attr.label) {
            (Some(path), None) => {
                if attr.unit.is_some()
                    || attr.refers_to.is_some()
                    || attr.help.is_some()
                    || attr.default.is_some()
                    || has_limits
                {
                    return Err(syn::Error::new_spanned(
                        path,
                        "a shared argument takes its unit, help, table, default and limits from the constant",
                    ));
                }
                quote!({
                    assert!(
                        #core::interface::same_text(#path.field.key, #key),
                        "the key of the shared argument is not the name of the struct field"
                    );
                    #path
                })
            }
            (None, Some(label)) => {
                let empty = LitStr::new("", Span::call_site());
                let unit = attr.unit.clone().unwrap_or_else(|| empty.clone());
                let help = attr.help.clone().unwrap_or(empty);
                if attr.refers_column.is_some() && attr.refers_to.is_none() {
                    return Err(syn::Error::new_spanned(
                        ident,
                        "column names a column of the table of refers_to: give refers_to too",
                    ));
                }
                // The table of the argument: its row keys, or the values of one of its columns.
                let refers: Vec<TokenStream> = attr
                    .refers_to
                    .iter()
                    .map(|table| match &attr.refers_column {
                        Some(column) => quote!(.refers_to_column(#table, #column)),
                        None => quote!(.refers_to_table(#table)),
                    })
                    .collect();
                if optional {
                    let Some(default) = &attr.default else {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "an optional argument needs #[arg(default = ...)]: the value in force when it is left out",
                        ));
                    };
                    let range = match (&attr.min, &attr.max, &attr.step) {
                        (Some(min), Some(max), Some(step)) => {
                            quote!(s = s.range(#min as f64, #max as f64, #step as f64);)
                        }
                        _ => quote!(),
                    };
                    quote!({
                        let mut s = #core::Setting::new(#key, #label, #unit, #default).help(#help);
                        #(s.field = s.field #refers;)*
                        #range
                        s
                    })
                } else {
                    quote!(#core::settings::Argument::new(
                        #core::Field::new(#key, #label, #unit) #(#refers)*
                    ).help(#help))
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "give #[arg(label = \"...\")] or #[arg(shared = PATH)]",
                ))
            }
        };
        list.push(quote!(&<#static_ty>::#cname as &dyn #core::settings::DeclareSetting));
        docs.push(doc.clone());
        if optional {
            consts.push(quote! {
                #[doc = #doc]
                pub const #cname: #core::Setting<#kind> = #value;
            });
            writes.push(quote! {
                if let ::core::option::Option::Some(v) = &self.#ident {
                    rec.set(&<#static_ty>::#cname.field, #core::interface::Cell::give(v));
                }
            });
            reads.push(quote! {
                #ident: ::core::option::Option::Some(
                    #core::interface::Cell::take(values.get(&<#static_ty>::#cname))
                )
            });
        } else {
            consts.push(quote! {
                #[doc = #doc]
                pub const #cname: #core::settings::Argument<#kind> = #value;
            });
            writes.push(quote! {
                rec.set(&<#static_ty>::#cname.field, #core::interface::Cell::give(&self.#ident));
            });
            reads.push(quote! {
                #ident: #core::interface::Cell::take(values.arg(&<#static_ty>::#cname)?)
            });
        }
    }
    let shape_name = name.to_string();
    let shape_doc = doc_of(&input.attrs);
    Ok(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #(#consts)*
        }
        impl #impl_generics #core::interface::Arguments for #name #ty_generics #where_clause {
            type Static = #static_ty;
            const SHAPE: #core::interface::ArgShape = #core::interface::ArgShape {
                name: #shape_name,
                doc: #shape_doc,
                args: &[#(#list),*],
                docs: &[#(#docs),*],
            };
            fn write(&self, rec: &mut #core::Record) {
                #(#writes)*
            }
        }
        impl #read_impl #core::interface::ReadArguments<#read_life> for #name #ty_generics #where_clause {
            fn read(
                values: &#read_life #core::Values,
            ) -> ::core::result::Result<Self, #core::CommandError> {
                ::core::result::Result::Ok(Self { #(#reads),* })
            }
        }
    })
}
