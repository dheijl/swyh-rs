// #![windows_subsystem = "windows"]  // enable to suppress println!

use fltk::{app, button::*, frame::*, window::*};
use futures::prelude::*;
use rupnp::ssdp::{SearchTarget, URN};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScanState {
    Started,
    Complete,
    Done,
}

#[derive(Debug)]
struct Renderer {
    dev_name: String,
    dev_model: String,
    dev_type: String,
    dev_url: String,
    svc_type: String,
    svc_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = app::App::default().with_scheme(app::Scheme::Gleam);
    let (sw, sh) = app::screen_size();
    let mut wind = Window::default()
        .with_size((sw as i32) / 3, (sh as i32) / 3)
        .with_label("UPNP/DLNA Renderers");

    let fw = (sw as i32) / 4;
    let fx = ((wind.width() - 30) / 2) - (fw / 2);
    let mut frame = Frame::new(fx, 5, fw, 30, "Scanning for UPNP rendering devices...")
        .with_align(Align::Center);
    frame.set_frame(FrameType::BorderBox);

    wind.make_resizable(true);
    wind.end();
    wind.show();
    app::wait_for(0.1)?;

    // Event handling channel
    let (s, r) = app::channel::<i32>();
    // the buttons with the discovered renderers
    let mut buttons: Vec<LightButton> = Vec::new();

    // build a list with renderers descovered on the network
    let renderers = discover().await?;
    // now create a button for each renderer
    let bwidth = frame.width() / 2; // button width
    let bheight = frame.height(); // button height
    let bx = ((wind.width() - 30) / 2) - (bwidth / 2); // button x offset
    let mut by = frame.y() + frame.height() + 10; // button y offset
    let mut bi = 0; // button index
    match renderers {
        Some(rs) => {
            for renderer in rs.iter() {
                let mut but = LightButton::default() // create the button
                    .with_size(bwidth, bheight)
                    .with_pos(bx, by)
                    .with_align(Align::Center)
                    .with_label(&format!("{} {}", renderer.dev_model, renderer.dev_name));
                but.emit(s, bi); // button click events arrive on a channel with the button index as message data
                wind.add(&but); // add the button to the window
                buttons.push(but); // and keep a reference to it
                bi += 1; // bump the button index
                by += bheight + 10; // and the button y offset
            }
        }
        None => {}
    }
    frame.set_label("Rendering Devices");
    wind.redraw();
    app::wait_for(0.1)?;

    while app.wait()? {
        match r.recv() {
            Some(i) => {
                // a button has been clicked
                let b = &buttons[i as usize]; // get a reference to the button that was clicked
                println!(
                    "Device button {} pushed, state = {}",
                    b.label(),
                    if b.is_on() { "ON" } else { "OFF" }
                );
            }
            None => (),
        }
    }
    Ok(())
}

const RENDERING_CONTROL: URN = URN::service("schemas-upnp-org", "RenderingControl", 1);

async fn discover() -> Result<Option<Vec<Renderer>>, rupnp::Error> {
    let mut renderers: Vec<Renderer> = Vec::new();

    println!("Starting renderer discovery");
    let search_target = SearchTarget::URN(RENDERING_CONTROL);
    let devices = rupnp::discover(&search_target, Duration::from_secs(3)).await?;
    pin_utils::pin_mut!(devices);

    loop {
        if let Some(device) = devices.try_next().await? {
            if device.services().len() > 0 {
                if let Some(service) = device.find_service(&RENDERING_CONTROL) {
                    //Some(service) => {
                    renderers.push(Renderer {
                        dev_name: device.friendly_name().to_string(),
                        dev_model: device.model_name().to_string(),
                        dev_type: device.device_type().to_string(),
                        dev_url: device.url().to_string(),
                        svc_id: service.service_type().to_string(),
                        svc_type: service.service_type().to_string(),
                    });
                    println!(
                        "Found renderer type={}, manufacturer={}, name={}, model={}, at url= {}",
                        device.device_type(),
                        device.manufacturer(),
                        device.friendly_name(),
                        device.model_name(),
                        device.url()
                    );
                    println!("  Service");
                    println!("    type: {}", service.service_type());
                    println!("    id:   {}", service.service_id());

                    let args = "<InstanceID>0</InstanceID><Channel>Master</Channel>";
                    match service.action(device.url(), "GetVolume", args).await {
                        Ok(response) => {
                            println!("Got response from {}", device.friendly_name());
                            let volume =
                                response.get("CurrentVolume").expect("Error getting volume");
                            println!("'{}' is at volume {}", device.friendly_name(), volume);
                        }
                        Err(err) => {
                            println!("Error '{}' in GetVolume", err);
                        }
                    }
                }
            } else {
                println!(
                    "No services: type={}, manufacturer={}, name={}, model={}, at url= {}",
                    device.device_type(),
                    device.manufacturer(),
                    device.friendly_name(),
                    device.model_name(),
                    device.url()
                );
            }
        } else {
            break;
        }
    }

    Ok(Some(renderers))
}
