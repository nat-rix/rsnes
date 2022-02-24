use std::collections::HashMap;
use std::path::{Path, PathBuf};
use toml::value::{Table, Value};

static CONFIG_FILE_PATHS: &'static [(bool, &'static str)] = &[
    (true, ".config/rsnes/config.toml"),
    (true, ".config/rsnes.toml"),
    (false, "/etc/rsnes.toml"),
];

#[derive(Debug)]
pub enum ConfigLoadError {
    Io(std::io::Error),
    De(toml::de::Error),
    WrongType {
        expected: &'static str,
        got: &'static str,
    },
    UnknownField(String),
    RequiredAttr {
        location: &'static str,
        attr: &'static str,
    },
    UnknownValue {
        field: &'static str,
        value: String,
    },
    UndefinedName {
        name: String,
        ty: &'static str,
    },
}

impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(fmt, "unable to read config file ({err})"),
            Self::De(err) => write!(fmt, "config file parsing error: {err}"),
            Self::WrongType { expected, got } => {
                write!(fmt, "expected type `{expected}`, got `{got}`")
            }
            Self::UnknownField(field) => {
                write!(fmt, "unknown field `{field}`")
            }
            Self::RequiredAttr { location, attr } => {
                write!(fmt, "missing attribute `{attr}` in `{location}`")
            }
            Self::UnknownValue { field, value } => {
                write!(fmt, "unknown value \"{value}\" for field `{field}`")
            }
            Self::UndefinedName { name, ty } => write!(fmt, "undefined {ty} `{name}`"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

macro_rules! getval {
    ($val:expr, $ty:ident) => {
        match $val {
            Value::$ty(val) => Ok(val),
            val => Err(ConfigLoadError::WrongType {
                expected: stringify!($ty),
                got: val.type_str(),
            }),
        }
    };
}

#[derive(Debug, Clone)]
pub struct ControllerProfileStandardScancodes {
    pub a: Option<u32>,
    pub b: Option<u32>,
    pub x: Option<u32>,
    pub y: Option<u32>,
    pub up: Option<u32>,
    pub left: Option<u32>,
    pub down: Option<u32>,
    pub right: Option<u32>,
    pub l: Option<u32>,
    pub r: Option<u32>,
    pub start: Option<u32>,
    pub select: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum ControllerProfile {
    Standard {
        scancodes: ControllerProfileStandardScancodes,
    },
    Mouse {
        xspeed: f64,
        yspeed: f64,
    },
}

impl ControllerProfile {
    fn load(map: &Table) -> Result<Self, ConfigLoadError> {
        let ty = getval!(
            map.get("type").ok_or(ConfigLoadError::RequiredAttr {
                location: "controller-profiles.*",
                attr: "type",
            })?,
            String
        )?;
        match ty.as_str() {
            "standard" => Self::load_standard(map),
            "mouse" => Self::load_mouse(map),
            _ => Err(ConfigLoadError::UnknownValue {
                field: "type",
                value: ty.clone(),
            }),
        }
    }

    fn load_mouse(map: &Table) -> Result<Self, ConfigLoadError> {
        macro_rules! getspeed {
            ($name:literal) => {{
                map.get($name)
                    .map(|val| getval!(val, Float).copied())
                    .transpose()?
                    .unwrap_or(1.0)
            }};
        }
        Ok(Self::Mouse {
            xspeed: getspeed!("xspeed"),
            yspeed: getspeed!("yspeed"),
        })
    }

    fn load_standard(map: &Table) -> Result<Self, ConfigLoadError> {
        if let Some(map) = map.get("scancodes") {
            macro_rules! getreq {
                ($name:literal) => {{
                    map.get($name)
                        .map(|val| getval!(val, Integer).map(|i| *i as u32))
                        .transpose()?
                }};
            }
            Ok(Self::Standard {
                scancodes: ControllerProfileStandardScancodes {
                    a: getreq!("A"),
                    b: getreq!("B"),
                    x: getreq!("X"),
                    y: getreq!("Y"),
                    up: getreq!("Up"),
                    down: getreq!("Down"),
                    left: getreq!("Left"),
                    right: getreq!("Right"),
                    l: getreq!("L"),
                    r: getreq!("R"),
                    start: getreq!("Start"),
                    select: getreq!("Select"),
                },
            })
        } else {
            Ok(Self::default_standard())
        }
    }

    fn default_standard() -> Self {
        Self::Standard {
            scancodes: ControllerProfileStandardScancodes {
                a: Some(0x24),
                b: Some(0x25),
                x: Some(0x26),
                y: Some(0x27),
                up: Some(0x11),
                left: Some(0x1e),
                down: Some(0x1f),
                right: Some(0x20),
                l: Some(0x10),
                r: Some(0x12),
                start: Some(0x38),
                select: Some(0x64),
            },
        }
    }

    pub fn handle_scancode(
        &self,
        scancode: u32,
        is_pressed: bool,
        controller: &mut rsnes::controller::Controller,
    ) -> bool {
        match self {
            Self::Standard {
                scancodes:
                    ControllerProfileStandardScancodes {
                        a,
                        b,
                        x,
                        y,
                        up,
                        left,
                        down,
                        right,
                        l,
                        r,
                        start,
                        select,
                    },
            } => {
                use rsnes::controller::buttons::*;
                let mut key = 0;
                for (code, button) in [
                    (a, A),
                    (b, B),
                    (x, X),
                    (y, Y),
                    (up, UP),
                    (left, LEFT),
                    (down, DOWN),
                    (right, RIGHT),
                    (l, L),
                    (r, R),
                    (start, START),
                    (select, SELECT),
                ]
                .into_iter()
                .filter_map(|(c, b)| c.map(|c| (c, b)))
                {
                    if code == scancode {
                        key = button;
                        break;
                    }
                }
                let handled = key > 0;
                if handled {
                    match controller {
                        rsnes::controller::Controller::Standard(controller) => {
                            if is_pressed {
                                controller.pressed_buttons |= key
                            } else {
                                controller.pressed_buttons &= !key
                            }
                        }
                        _ => (),
                    }
                }
                handled
            }
            _ => false,
        }
    }

    pub fn handle_mouse_button(
        &self,
        button: winit::event::MouseButton,
        is_pressed: bool,
        controller: &mut rsnes::controller::Controller,
    ) {
        match controller {
            rsnes::controller::Controller::Mouse(mouse) => match button {
                winit::event::MouseButton::Left => mouse.left_button = is_pressed,
                winit::event::MouseButton::Right => mouse.right_button = is_pressed,
                _ => (),
            },
            _ => (),
        }
    }

    pub fn handle_mouse_move(
        &self,
        dx: f64,
        dy: f64,
        controller: &mut rsnes::controller::Controller,
    ) {
        match self {
            Self::Mouse { xspeed, yspeed } => match controller {
                rsnes::controller::Controller::Mouse(mouse) => {
                    let [dx, dy] = [dx * xspeed, dy * yspeed];
                    let off =
                        [dx, dy].map(|v| v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32);
                    mouse.add_offset(off)
                }
                _ => (),
            },
            _ => (),
        }
    }

    pub fn is_mouse(&self) -> bool {
        matches!(self, Self::Mouse { .. })
    }
}

impl Default for ControllerProfile {
    fn default() -> Self {
        Self::default_standard()
    }
}

pub fn controller_profile_to_port(
    profile: Option<&ControllerProfile>,
) -> rsnes::controller::ControllerPort {
    use rsnes::controller::{Controller, ControllerPort, Mouse, StandardController};
    ControllerPort::new(match profile {
        None => Controller::None,
        Some(ControllerProfile::Standard { .. }) => Controller::Standard(StandardController::new()),
        Some(ControllerProfile::Mouse { .. }) => Controller::Mouse(Mouse::default()),
    })
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub port1: Option<String>,
    pub port2: Option<String>,
    pub region: rsnes::cartridge::CountryFrameRate,
    pub threaded: bool,
}

impl Profile {
    fn load(map: &Table) -> Result<Self, ConfigLoadError> {
        macro_rules! get_port {
            ($name:literal) => {
                map.get($name)
                    .map(|v| getval!(v, String))
                    .transpose()?
                    .cloned()
            };
        }
        let port1 = get_port!("port1");
        let port2 = get_port!("port2");
        let region = map
            .get("region")
            .map(|v| getval!(v, String))
            .transpose()?
            .and_then(|region| match region.as_str() {
                "auto" => Some(rsnes::cartridge::CountryFrameRate::Any),
                "pal" => Some(rsnes::cartridge::CountryFrameRate::Pal),
                "ntsc" => Some(rsnes::cartridge::CountryFrameRate::Ntsc),
                _ => None,
            })
            .unwrap_or(rsnes::cartridge::CountryFrameRate::Any);
        let threaded = map
            .get("threaded")
            .map(|v| getval!(v, Boolean))
            .transpose()?
            .copied()
            .unwrap_or(true);
        Ok(Self {
            port1,
            port2,
            region,
            threaded,
        })
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            port1: Some(String::from("default")),
            port2: None,
            region: rsnes::cartridge::CountryFrameRate::Any,
            threaded: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    default_profile: String,
    profiles: HashMap<String, Profile>,
    controller_profiles: HashMap<String, ControllerProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_profile: String::from("default"),
            profiles: [(String::from("default"), Profile::default())].into(),
            controller_profiles: [(String::from("default"), ControllerProfile::default())].into(),
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>, verbose: bool) -> Result<Self, ConfigLoadError> {
        if let Some(path) = path.or_else(Self::seek_config_path) {
            if verbose {
                println!("[info] loading config file `{}`", path.display());
            }
            Self::load_from_file(path)
        } else {
            Ok(Self::default())
        }
    }

    fn load_controller_profiles(
        map: &Table,
    ) -> Result<HashMap<String, ControllerProfile>, ConfigLoadError> {
        map.into_iter()
            .map(|(key, val)| {
                getval!(val, Table)
                    .and_then(ControllerProfile::load)
                    .map(|val| (key.clone(), val))
            })
            .collect()
    }

    fn load_profiles(map: &Table) -> Result<HashMap<String, Profile>, ConfigLoadError> {
        map.into_iter()
            .map(|(key, val)| {
                getval!(val, Table)
                    .and_then(Profile::load)
                    .map(|val| (key.clone(), val))
            })
            .collect()
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigLoadError> {
        let main: Table =
            toml::de::from_str(&std::fs::read_to_string(path)?).map_err(ConfigLoadError::De)?;
        let mut controller_profiles = Default::default();
        let mut profiles = Default::default();
        let mut default_profile = None;
        for (key, val) in main.iter() {
            match key.as_str() {
                "default-profile" => {
                    default_profile = Some(getval!(val, String)?.clone());
                }
                "profiles" => profiles = Self::load_profiles(getval!(val, Table)?)?,
                "controller-profiles" => {
                    controller_profiles = Self::load_controller_profiles(getval!(val, Table)?)?
                }
                _ => return Err(ConfigLoadError::UnknownField(key.clone())),
            }
        }
        let default_profile = default_profile.ok_or_else(|| ConfigLoadError::RequiredAttr {
            location: "root",
            attr: "default-profile",
        })?;
        let slf = Self {
            default_profile,
            profiles,
            controller_profiles,
        };
        slf.validate_names()?;
        Ok(slf)
    }

    fn validate_names(&self) -> Result<(), ConfigLoadError> {
        if !self.profiles.contains_key(&self.default_profile) {
            return Err(ConfigLoadError::UndefinedName {
                name: self.default_profile.clone(),
                ty: "profile",
            });
        }
        for profile in self.profiles.values() {
            for port in profile.port1.iter().chain(profile.port2.iter()) {
                if !self.controller_profiles.contains_key(port) {
                    return Err(ConfigLoadError::UndefinedName {
                        name: port.clone(),
                        ty: "controller profile",
                    });
                }
            }
        }
        Ok(())
    }

    pub fn seek_config_path() -> Option<PathBuf> {
        CONFIG_FILE_PATHS
            .iter()
            .filter_map(|&(with_home, path)| {
                if with_home {
                    std::env::var_os("HOME").map(|home| Path::new(&home).join(path))
                } else {
                    Some(PathBuf::from(path))
                }
            })
            .find(|path| path.is_file())
    }

    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn get_default_profile(&self) -> &Profile {
        self.profiles.get(&self.default_profile).unwrap()
    }

    pub fn get_controller_profiles<'a>(
        &'a self,
        profile: &Profile,
    ) -> [Option<&'a ControllerProfile>; 2] {
        [&profile.port1, &profile.port2].map(|name| {
            name.as_ref()
                .map(|name| self.controller_profiles.get(name).unwrap())
        })
    }
}
