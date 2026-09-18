//! Music out of a real device.
//!
//! The mixer in `hb-audio` produces samples and knows nothing about hardware;
//! this hands them to cpal. It is deliberately the only place in the workspace
//! that touches an audio API.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hb_audio::{Mixer, Module};

pub struct Music {
    mixer: Arc<Mutex<Option<Mixer>>>,
    _stream: cpal::Stream,
    pub rate: u32,
}

impl Music {
    /// Opens the default output and starts a silent stream.
    pub fn open() -> Result<Music, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no default audio output")?;
        let config = device
            .default_output_config()
            .map_err(|e| format!("no output config: {e}"))?;
        let rate = config.sample_rate();
        let channels = config.channels() as usize;
        let mixer: Arc<Mutex<Option<Mixer>>> = Arc::new(Mutex::new(None));
        let shared = Arc::clone(&mixer);

        // The mixer renders stereo; anything else is spread from it.
        let mut scratch: Vec<i16> = Vec::new();
        let stream = device
            .build_output_stream(
                config.clone().into(),
                move |out: &mut [f32], _| {
                    let frames = out.len() / channels;
                    scratch.resize(frames * 2, 0);
                    scratch.fill(0);
                    if let Ok(mut guard) = shared.lock() {
                        if let Some(mixer) = guard.as_mut() {
                            mixer.render(&mut scratch);
                        }
                    }
                    for (frame, slot) in out.chunks_mut(channels).enumerate() {
                        let left = scratch[frame * 2] as f32 / 32768.0;
                        let right = scratch[frame * 2 + 1] as f32 / 32768.0;
                        for (i, sample) in slot.iter_mut().enumerate() {
                            *sample = if i % 2 == 0 { left } else { right };
                        }
                    }
                },
                |err| eprintln!("audio: {err}"),
                None,
            )
            .map_err(|e| format!("could not open the output stream: {e}"))?;
        stream.play().map_err(|e| format!("could not start audio: {e}"))?;
        Ok(Music { mixer, _stream: stream, rate })
    }

    pub fn play(&self, module: Module) {
        if let Ok(mut guard) = self.mixer.lock() {
            *guard = Some(Mixer::new(module, self.rate));
        }
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.mixer.lock() {
            *guard = None;
        }
    }

    pub fn playing(&self) -> bool {
        self.mixer.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}
