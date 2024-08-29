use core::f32;
use std::{
    array,
    f64::consts::PI,
    io::{Seek, Write},
    ops::{Add, Mul},
    path::PathBuf,
};

use clap::Parser;
use hound::{WavSpec, WavWriter};
use image::{imageops::resize, ImageReader, Rgba};

#[derive(Debug, Clone)]
pub struct FreqDur {
    /// Transmit a frequency (in hertz) for a certain duration (in milliseconds)
    frequency: f32,
    duration: f32,
}
pub const fn transmit(freq: f32, dur: f32) -> FreqDur {
    FreqDur {
        frequency: freq,
        duration: dur,
    }
}
const HEADER: &[FreqDur] = &[
    transmit(500.0, 1000.0),
    transmit(1900.0, 300.0),
    transmit(1200.0, 10.0),
    transmit(1900.0, 300.0),
    transmit(1200.0, 10.0),
];

fn build_header(vis_code: u8, parity_even: bool) -> [FreqDur; 13] {
    fn digital(bit: bool) -> FreqDur {
        if bit {
            transmit(1100.0, 30.0)
        } else {
            transmit(1300.0, 30.0)
        }
    }
    array::from_fn(|idx| match idx {
        0..=3 => HEADER[idx].clone(),
        4..=10 => digital((vis_code >> (idx - 4)) != 0),
        11 => digital(parity_even),
        12 => transmit(1200.0, 30.0),
        _ => unreachable!(),
    })
}
pub struct Scans {
    pixel_dur: f32,
    red_samples: Vec<FreqDur>,
    green_samples: Vec<FreqDur>,
    blue_samples: Vec<FreqDur>,
}
pub const TAU: f64 = PI * 2.0;
impl Scans {
    fn new(pixel_dur: f32) -> Self {
        Self {
            pixel_dur,
            red_samples: Vec::new(),
            green_samples: Vec::new(),
            blue_samples: Vec::new(),
        }
    }
    fn clear(&mut self) {
        self.red_samples.clear();
        self.green_samples.clear();
        self.blue_samples.clear();
    }
    fn push_pixel(&mut self, pixel: &Rgba<u8>) {
        fn color_to_freq(col: u8) -> f32 {
            (col as f32).mul((2300.0 - 1500.0) / 255.0).add(1500.0)
        }
        self.red_samples
            .push(transmit(color_to_freq(pixel.0[0]), self.pixel_dur));
        self.green_samples
            .push(transmit(color_to_freq(pixel.0[1]), self.pixel_dur));
        self.blue_samples
            .push(transmit(color_to_freq(pixel.0[2]), self.pixel_dur));
    }
}

fn main() {
    let args = Args::parse();
    let mut buf = Vec::new();
    let image = ImageReader::open(args.image).unwrap().decode().unwrap();
    let scaled = resize(&image, 320, 256, image::imageops::FilterType::Gaussian);
    let header = build_header(60, false);

    buf.extend_from_slice(&header);

    buf.push(transmit(1200.0, 9.0));
    let mut samples = Scans::new(0.432);
    for row in scaled.rows() {
        for pixel in row {
            samples.push_pixel(pixel);
        }
        //seperator
        buf.push(transmit(1500.0, 1.5));
        // green scan
        buf.extend_from_slice(&samples.green_samples);
        //separator
        buf.push(transmit(1500.0, 1.5));
        // blue scan
        buf.extend_from_slice(&samples.blue_samples);
        //sync pulse
        buf.push(transmit(1200.0, 9.0));
        //porch
        buf.push(transmit(1500.0, 1.5));
        //red scan
        buf.extend_from_slice(&samples.red_samples);
        samples.clear();
    }

    let spec = WavSpec {
        channels: 1,
        sample_rate: 11025 * 4,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = WavWriter::create(args.out_path, spec).unwrap();
    render(buf, &mut writer, 400);
    writer.finalize().unwrap();
}
fn render(items: Vec<FreqDur>, writer: &mut WavWriter<impl Seek + Write>, baseband: u32) {
    let rate = writer.spec().sample_rate;
    let dt = 1.0 / rate as f64;
    let mut p = dt / 2.0;
    for item in items {
        for _ in 0..(item.duration as f64 / 1000.0 / dt) as u32 {
            writer
                .write_sample(
                    p.mul(TAU)
                        .mul(item.frequency as f64)
                        .sin()
                        .mul(0.8)
                        .add((baseband as f64).mul(TAU).sin().mul(0.2)) as f32,
                )
                .unwrap();
            p += dt;
        }
    }
}
#[derive(clap::Parser)]
pub struct Args {
    image: PathBuf,
    #[clap(default_value = "./out.wav")]
    out_path: PathBuf,
}
