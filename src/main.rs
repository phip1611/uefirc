#![no_main]
#![no_std]
#[allow(dead_code)]

mod tcpv4;
mod ipv4;
mod event;
mod connection;
mod ui;
mod app;

extern crate alloc;

use log::info;
use uefi::prelude::*;
use crate::app::IrcClient;
use crate::ui::set_resolution;

#[entry]
fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let bs = system_table.boot_services();
    let bs: &'static BootServices = unsafe {
        core::mem::transmute(bs)
    };

    let graphics_protocol = set_resolution(bs, (1920, 1080)).unwrap();

    /*
    let mut client = IrcClient::new(bs);
    loop {
        client.step();
    }

     */

    /*
    connection.transmit(b"NICK phillip-testing\r\n");
    connection.transmit(b"USER phillip-testing O * :phillip@axleos.com\r\n");
    loop {
        connection.step();
    }

     */
    loop{}
}
