//! The tests of the satellite communications family: the family rules, and one module for each stack.

mod common;
/// The rules every stack of the family follows.
mod family;

mod aero;
mod argos;
mod bgan;
mod cubesat;
mod cubesat_tx;
mod fleet;
mod globalstar;
mod goes;
mod hrdcp;
mod inmarsat;
mod inmarsatc;
mod iridium;
mod lorasat;
mod orbcomm;
mod psk;
mod qo100;
mod qo100_tx;
mod sarsat;
mod sarsat_weak;
