#![no_main]

use alloc::rc::Rc;
use alloc::vec::Vec;
use agx_definitions::{Color, Drawable, NestedLayerSlice, Point, Rect, StrokeThickness};
#[allow(dead_code)]

use agx_definitions::Size;
use libgui::{AwmWindow, KeyCode};
use libgui::ui_elements::UIElement;
use libgui::view::View;
use log::info;
use ttf_renderer::Font;
use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::Key;
use uefi::table::boot::{OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol};
use crate::app::IrcClient;
use crate::fs::read_file;
use crate::gui::{ContentView, InputBoxView, TitleView};
use crate::ui::set_resolution;

struct App {
    font_regular: Font,
    window: Rc<AwmWindow>,
    content_view: Rc<ContentView>,
    input_box_view: Rc<InputBoxView>,
}

impl App {
    fn new(
        resolution: Size,
        font_regular: Font,
    ) -> Self {
        let window = AwmWindow::new(resolution);
        let title_sizer = |v: &View, superview_size: Size| {
            Rect::with_size(
                Size::new(
                    superview_size.width,
                    (superview_size.height as f64 * 0.084) as _,
                )
            )
        };

        let title_sizer_clone = title_sizer.clone();
        let content_sizer = move |v: &View, superview_size: Size| {
            let title_frame = title_sizer_clone(v, superview_size);
            Rect::from_parts(
                Point::new(0, title_frame.max_y()),
                Size::new(
                    superview_size.width,
                    (superview_size.height as f64 * 0.82) as _,
                )
            )
        };

        let content_sizer_clone = content_sizer.clone();
        let input_box_sizer = move |v: &View, superview_size: Size| {
            let content_frame = content_sizer_clone(v, superview_size);
            Rect::from_parts(
                Point::new(
                    0,
                    content_frame.max_y(),
                ),
                Size::new(
                    superview_size.width,
                    (superview_size.height as f64 * 0.1) as _,
                )
            )
        };

        let content = ContentView::new(
            font_regular.clone(),
            Size::new(20, 20),
            content_sizer,
        );

        let input_box = InputBoxView::new(
            font_regular.clone(),
            Size::new(24, 24),
            move |v, s| input_box_sizer(v, s),
        );

        let title = TitleView::new(
            font_regular.clone(),
            Size::new(32, 32),
            move |v, s| title_sizer(v, s),
        );
        Rc::clone(&window).add_component(Rc::clone(&title) as Rc<dyn UIElement>);
        Rc::clone(&window).add_component(Rc::clone(&content) as Rc<dyn UIElement>);
        Rc::clone(&window).add_component(Rc::clone(&input_box) as Rc<dyn UIElement>);

        Self {
            font_regular,
            window,
            content_view: content,
            input_box_view: input_box,
        }
    }

    pub fn handle_recv_data(&self, recv_data: &[u8]) {
        let recv_as_str = core::str::from_utf8(recv_data).unwrap();
        for ch in recv_as_str.chars() {
            self.content_view.view.draw_char_and_update_cursor(ch, Color::black());
        }
        let cursor_pos = self.content_view.view.cursor_pos.borrow().1;
        let viewport_height = self.content_view.frame().height();
        *self.content_view.view.view.layer.scroll_offset.borrow_mut() = Point::new(
            cursor_pos.x,
            cursor_pos.y - viewport_height + 32,
        );
    }
}

pub fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let bs = system_table.boot_services();
    let bs: &'static BootServices = unsafe {
        core::mem::transmute(bs)
    };

    // Disable the UEFI watchdog timer as we want to run indefinitely
    bs.set_watchdog_timer(
        0,
        0x1ffff,
        None,
    ).expect("Failed to disable watchdog timer");

    info!("Parsing fonts...");
    //let font_regular = ttf_renderer::parse(&read_file(bs, "EFI\\Boot\\sf_pro.ttf"));
    /*
    Nice:
    Bodoni
    DIN
    BigCaslon
    Chancery
     */
    let font_regular = ttf_renderer::parse(&read_file(bs, "EFI\\Boot\\BigCaslon.ttf"));
    let font_arial = ttf_renderer::parse(&read_file(bs, "EFI\\Boot\\Chancery.ttf"));
    let font_italic = ttf_renderer::parse(&read_file(bs, "EFI\\Boot\\chancery.ttf"));
    info!("All done!");

    //let resolution = Size::new(1920, 1080);
    let resolution = Size::new(1360, 768);
    let mut graphics_protocol = set_resolution(
        bs,
        resolution,
    ).unwrap();

    /*
    let mut irc_client = IrcClient::new(bs);
    {
        irc_client.connect_to_server();
        irc_client.set_nickname("phillip-testing\r\nUSER phillip-testing 0 * :phillip@axleos.com\r\n");
        //let data = format!("/USER {nickname} 0 * :{real_name}\r\n").into_bytes();
        //irc_client.set_user("phillip-testing", "phillip@axleos.com");
    }
    {
        let conn = irc_client.active_connection.as_mut();
        let conn = conn.unwrap();
        Rc::clone(&conn).set_up_receive_signal_handler();
    }
    */
    // Theory: we need to do the same careful stuff for transmit as for receive
    // To test, going to try to only set up the RX handler after doing our initial transmits

    let app = App::new(resolution, font_regular);
    let mut currently_held_key: Option<KeyCode> = None;

    let pointer_handle = bs.get_handle_for_protocol::<Pointer>().expect("Failed to find handle for Pointer protocol");
    let mut pointer = bs.open_protocol_exclusive::<Pointer>(pointer_handle).expect("failed to open proto");
    pointer.reset(false).expect("Failed to reset cursor");

    let pointer_resolution = pointer.mode().resolution;
    let pointer_resolution = Point::new(
        pointer_resolution[0] as _,
        pointer_resolution[1] as _,
    );
    // Start off the mouse in the middle of the screen
    let mut current_pointer_pos = Point::new(resolution.mid_x(), resolution.mid_y());

    loop {
        /*
        irc_client.step();
        let mut active_connection = irc_client.active_connection.as_mut();
        let recv_buffer = &active_connection.expect("Expected an active connection").recv_buffer;
        let recv_data = recv_buffer.lock().borrow_mut().drain(..).collect::<Vec<u8>>();
        //println!("Got recv data");
        main_view.handle_recv_data(&recv_data);
        */
        //println!("Got recv data");
        let key_held_on_this_iteration = {
            let maybe_key = system_table.stdin().read_key().expect("Failed to poll for a key");
            match maybe_key {
                None => None,
                Some(key) => {
                    let key_as_u16 = match key {
                        Key::Special(scancode) => {
                            scancode.0
                        }
                        Key::Printable(char_u16) => {
                            char::from(char_u16) as _
                        }
                    };
                    Some(KeyCode(key_as_u16 as _))
                }
            }
        };

        // Are we changing state in any way?
        //println!("Got key {key_held_on_this_iteration:?}");
        if key_held_on_this_iteration != currently_held_key {
            // Are we switching away from a held key?
            if currently_held_key.is_some() {
                app.window.handle_key_released(currently_held_key.unwrap());
            }
            if key_held_on_this_iteration.is_some() {
                // Inform the window that a new key is held
                app.window.handle_key_pressed(key_held_on_this_iteration.unwrap());
            }
            // And update our state to track that this key is currently held
            currently_held_key = key_held_on_this_iteration;
        }

        // Handle mouse updates
        let pointer_updates = pointer.read_state().expect("Failed to read pointer state");
        if let Some(pointer_updates) = pointer_updates {
            let rel_x = pointer_updates.relative_movement[0] as isize / pointer_resolution.x;
            let rel_y = pointer_updates.relative_movement[1] as isize /  pointer_resolution.y;
            current_pointer_pos.x += rel_x;
            current_pointer_pos.y += rel_y;
        }

        app.window.draw();

        let window_slice = app.window.get_slice();
        let cursor_frame = Rect::from_parts(
            current_pointer_pos,
            Size::new(15, 15),
        );
        // Inner cursor
        window_slice.fill_rect(
            cursor_frame,
            Color::new(66, 206, 245),
            StrokeThickness::Filled,
        );
        // Black outline
        window_slice.fill_rect(
            cursor_frame,
            Color::new(20, 20, 20),
            StrokeThickness::Width(3),
        );
        render_window_to_display(&app.window, &mut graphics_protocol);
    }
    /*
    let screen = Screen::new(
        resolution,
        graphics_protocol,
        font_regular,
        font_italic,
        irc_client,
    );
    loop {
        screen.step();
    }

     */

    /*
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

fn render_window_to_display(
    window: &AwmWindow,
    graphics_protocol: &mut ScopedProtocol<GraphicsOutput>,
) {
    let layer = window.layer.borrow_mut();
    let pixel_buffer = layer.framebuffer.borrow_mut();

    let buf_as_blt_pixel = unsafe {
        let buf_as_u8 = pixel_buffer;
        let len = buf_as_u8.len() / 4;
        let capacity = len;

        let buf_as_blt_pixels = buf_as_u8.as_ptr() as *mut BltPixel;
        Vec::from_raw_parts(
            buf_as_blt_pixels,
            len,
            capacity,
        )
    };

    let resolution = window.frame().size;
    graphics_protocol.blt(
        BltOp::BufferToVideo {
            buffer: &buf_as_blt_pixel,
            src: BltRegion::Full,
            dest: (0, 0),
            dims: (resolution.width as _, resolution.height as _),
        }
    ).expect("Failed to blit screen");

    // Forget our re-interpreted vector of pixel data, as it's really owned by the window
    core::mem::forget(buf_as_blt_pixel);
}
