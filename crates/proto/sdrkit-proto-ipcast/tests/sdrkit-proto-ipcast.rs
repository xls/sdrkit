//! The one test program of sdrkit-proto-ipcast. Each module tests one module of the crate against its standard.

#[path = "ipcast/adapt.rs"]
mod adapt;
#[path = "ipcast/alloc.rs"]
mod alloc;
#[path = "ipcast/alp.rs"]
mod alp;
#[path = "ipcast/bbheader.rs"]
mod bbheader;
#[path = "ipcast/components.rs"]
mod components;
#[path = "ipcast/filecast.rs"]
mod filecast;
#[path = "ipcast/gse.rs"]
mod gse;
#[path = "ipcast/gzip.rs"]
mod gzip;
#[path = "ipcast/ip.rs"]
mod ip;
#[path = "ipcast/lct.rs"]
mod lct;
#[path = "ipcast/mp4.rs"]
mod mp4;
#[path = "ipcast/mpe.rs"]
mod mpe;
#[path = "ipcast/pcap.rs"]
mod pcap;
#[path = "ipcast/route.rs"]
mod route;
#[path = "ipcast/sls.rs"]
mod sls;
