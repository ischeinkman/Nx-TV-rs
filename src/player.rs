
use decoder_shim;


use sdl2;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::video::{Window, WindowContext};
use sdl2::render::{Texture, TextureCreator, WindowCanvas, };
use sdl2::{ AudioSubsystem, VideoSubsystem, EventPump, JoystickSubsystem };
use sdl2::audio::{ AudioCallback, AudioSpecDesired };
use sdl2::event::{Event, EventType};
use decoder_shim::{Frame, Codec, VideoDecoder};
use image::{RgbaImage, ImageBuffer, Rgba};

struct CanvasWrapper{
    canvas : Canvas<Window>,
    texture_creator : TextureCreator<WindowContext>,
    texture : Texture,
}

pub trait NewCanvas {
    fn new_canvas(& self, w: usize, h: usize, name: &str) -> Result<CanvasWrapper, String>;
}

impl NewCanvas for VideoSubsystem {
    fn new_canvas(& self, w: usize, h: usize, name: &str) -> Result<CanvasWrapper, String> {
        let window = self.window(name, w as u32, h as u32)
            .position_centered()
            .opengl()
            .build()
            .unwrap();
        let canvas = window.into_canvas().build().unwrap();
        let texture_creator = canvas.texture_creator();
        let texture =  match texture_creator.create_texture_streaming(PixelFormatEnum::RGBA8888, w as u32, h as u32) {
            Ok(r) => r, 
            Err(e) => {
                return Err(format!("Error making texture: {}", e));
            }
        };

        Ok(CanvasWrapper {
            canvas, 
            texture_creator,
            texture ,
        })
    }
}

trait Blit {
    fn blit(&mut self, frame: &Frame);
}

impl Blit for CanvasWrapper {
    fn blit(&mut self, frame: &Frame) {
        let (w, h) = self.canvas.window().size();

        self.texture.update(None, frame.rgba_buff(), frame.width() as usize * 4);

        self.canvas.clear();
        match self.canvas.copy(&self.texture, None, None) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error in copy: {:?}", e);
            }
        };
        self.canvas.present();
    }
}

use std::time;


pub fn play_video<F : Frame, P : VideoDecoder + Iterator<Item=F>>(mut codec : P) {
    let sdl_context = sdl2::init().unwrap();
    let video= sdl_context.video().unwrap();
    let audio = sdl_context.audio().unwrap();
    let mut stick = sdl_context.joystick().unwrap();
    let mut controller = sdl_context.game_controller().unwrap();
    stick.set_event_state(true);
    controller.set_event_state(true);
    
    let mut event = sdl_context.event_pump().unwrap();

    let mut v_out = match video.new_canvas(
        1920, 
        1080, 
        ""
    ) {
        Ok(c) => c, 
        Err(e) => {
            eprintln!("Error on canvas creation: {}", e);
            return;
        }
    };
    let mut prev_frame_time = time::Instant::now();
    while let Some(frame) = codec.next() {
       for evt in event.poll_iter() {
           match evt {
                Event::Window{win_event : wevt, ..} => {
                    eprintln!("Got Window event: {:?}", wevt);
                }, 
                Event::JoyButtonDown{button_idx, which, timestamp : _} => {
                    eprintln!("Got Joy Button: {};{}", which, button_idx);
                }, 
                Event::FingerDown{x, y, dx, dy, pressure, touch_id, finger_id, ..} => {
                    eprintln!("Got finger down: {{ x: {}, y : {}, dx : {}, dy : {}, pressure : {}, touch_id : {}, finger_id : {} }}", x, y, dx, dy, pressure, touch_id, finger_id);
                },
                Event::FingerMotion{x, y, dx, dy, pressure, touch_id, finger_id, ..} => {
                    eprintln!("Got finger motion: {{ x: {}, y : {}, dx : {}, dy : {}, pressure : {}, touch_id : {}, finger_id : {} }}", x, y, dx, dy, pressure, touch_id, finger_id);
                },
                e => {
                    eprintln!("Got event: {:?}", e);
                }
            }
        }
        //let wait_time = time::Duration::from_nanos(frame.nanos_from_prev());
        //while time::Instant::now() - prev_frame_time < wait_time {

        //};
        //eprintln!("Blitting frame.");
        v_out.blit(&frame);
        //prev_frame_time = time::Instant::now();
    }

}