use serde::Deserialize;

use enum_iterator::IntoEnumIterator;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use rodio;
use rodio::Source;
use std::convert::AsRef;

//---------------------------------------------------

struct SoundImpl(Arc<Vec<u8>>);

impl AsRef<[u8]> for SoundImpl {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl SoundImpl {
    fn load(path: &Path) -> io::Result<SoundImpl> {
        let mut buf = Vec::new();
        let mut file = File::open(path)?;
        file.read_to_end(&mut buf)?;
        Ok(SoundImpl(Arc::new(buf)))
    }
    fn cursor(self: &Self) -> io::Cursor<SoundImpl> {
        io::Cursor::new(SoundImpl(self.0.clone()))
    }
    fn decoder(self: &Self) -> rodio::Decoder<io::Cursor<SoundImpl>> {
        rodio::Decoder::new(self.cursor()).unwrap()
    }
}

//---------------------------------------------------

lazy_static::lazy_static! {
    static ref KSOUND_FILENAMES: HashMap<KSnd, &'static str> = {
        [
            (KSnd::Click1, "click1.wav"),
            (KSnd::Click2, "click2.wav"),
            (KSnd::Sticky, "sticky.wav"),
            (KSnd::Error, "error.wav"),
        ].iter().cloned().collect()
    };
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, IntoEnumIterator, Deserialize)]
pub enum KSnd {
    Click1,
    Click2,
    Sticky,
    Error,
}

pub struct Dj {
    dev: rodio::Device,
    ksnds: HashMap<KSnd, SoundImpl>,
    custom_snds: HashMap<String, SoundImpl>,
}

impl Dj {
    fn make_ksnds(assets_path: &Path) -> HashMap<KSnd, SoundImpl> {
        let snds_dir = Path::new(assets_path).join("sounds");
        let mut out: HashMap<KSnd, SoundImpl> = HashMap::new();
        for snd in KSnd::into_enum_iter() {
            let path = snds_dir.join(&KSOUND_FILENAMES[&snd]);
            out.insert(snd, SoundImpl::load(&path).unwrap());
        }
        out
    }

    pub fn new(assets_path: &Path) -> Self {
        let dev = rodio::default_output_device().expect("Failed to open the default sound device");
        let ksnds = Self::make_ksnds(assets_path);
        Self {
            dev,
            ksnds,
            custom_snds: HashMap::new(),
        }
    }

    pub fn play(&self, snd: KSnd) {
        let snd = &self.ksnds[&snd];
        rodio::play_raw(&self.dev, snd.decoder().convert_samples());
    }

    pub fn play_custom(&mut self, path: &String) {
        if !self.custom_snds.contains_key(path) {
            let _path = Path::new(path);
            self.custom_snds
                .insert(path.clone(), SoundImpl::load(&_path).unwrap());
        }

        let snd = &self.custom_snds[path];
        rodio::play_raw(&self.dev, snd.decoder().convert_samples());
    }
}
