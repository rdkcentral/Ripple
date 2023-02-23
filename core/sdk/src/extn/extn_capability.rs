use crate::utils::error::RippleError;

/// Contains enums and structs for Extn
/// Types
/// Class
/// Capability enclosure
///
///
///
#[derive(Debug, Clone, PartialEq)]
pub enum ExtnClass {
    Gateway,
    Device,
    DataGovernance,
    Distributor,
    Rpc,
    Jsonrpsee,
    Launcher,
    Internal,
}

impl ToString for ExtnClass {
    fn to_string(&self) -> String {
        match self {
            Self::Gateway => "gateway".into(),
            Self::Device => "device".into(),
            Self::DataGovernance => "data-governance".into(),
            Self::Distributor => "distributor".into(),
            Self::Rpc => "rpc".into(),
            Self::Jsonrpsee => "jsonrpsee".into(),
            Self::Launcher => "launcher".into(),
            Self::Internal => "internal".into(),
        }
    }
}

impl ExtnClass {
    pub fn get(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "device" => Some(Self::Device),
            "data-governance" => Some(Self::DataGovernance),
            "distributor" => Some(Self::Distributor),
            "rpc" => Some(Self::Rpc),
            "jsonrpsee" => Some(Self::Jsonrpsee),
            "launcher" => Some(Self::Launcher),
            "internal" => Some(Self::Internal),
            "gateway" => Some(Self::Gateway),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtnType {
    Main,
    Channel,
    Extn,
}

impl ToString for ExtnType {
    fn to_string(&self) -> String {
        match self {
            Self::Main => "main".into(),
            Self::Channel => "channel".into(),
            Self::Extn => "extn".into(),
        }
    }
}

impl ExtnType {
    pub fn get(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "main" => Some(Self::Main),
            "channel" => Some(Self::Channel),
            "extn" => Some(Self::Extn),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtnClassType {
    _type: ExtnType,
    class: ExtnClass,
}

impl ExtnClassType {
    pub fn new(_type: ExtnType, class: ExtnClass) -> ExtnClassType {
        ExtnClassType { _type, class }
    }

    pub fn get_cap(&self, service: String) -> ExtnCapability {
        ExtnCapability {
            _type: self._type.clone(),
            class: self.class.clone(),
            service,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtnCapability {
    _type: ExtnType,
    class: ExtnClass,
    service: String,
}

impl ToString for ExtnCapability {
    fn to_string(&self) -> String {
        let r = format!(
            "ripple:{}:{}:{}",
            self._type.to_string(),
            self.class.to_string(),
            self.service
        );
        r
    }
}

impl TryFrom<String> for ExtnCapability {
    type Error = RippleError;

    fn try_from(cap: String) -> Result<Self, Self::Error> {
        let c_a = cap.split(":");
        if c_a.count() > 3 {
            let c_a: Vec<&str> = cap.split(":").collect();
            match c_a.get(0).unwrap().to_lowercase().as_str() {
                "ripple" => {}
                _ => return Err(RippleError::ParseError),
            }
            let _type = ExtnType::get(c_a.get(1).unwrap());
            if _type.is_none() {
                return Err(RippleError::ParseError);
            }
            let _type = _type.unwrap();

            let class = ExtnClass::get(c_a.get(2).unwrap());
            if class.is_none() {
                return Err(RippleError::ParseError);
            }
            let class = class.unwrap();

            let service = c_a.get(3);
            if service.is_none() {
                return Err(RippleError::ParseError);
            }
            let service = String::from(*service.unwrap());

            return Ok(ExtnCapability {
                _type,
                class,
                service,
            });
        }

        Err(RippleError::ParseError)
    }
}

impl ExtnCapability {
    pub fn get_main_target(service: String) -> ExtnCapability {
        ExtnCapability {
            _type: ExtnType::Main,
            class: ExtnClass::Internal,
            service,
        }
    }

    pub fn is_device_channel(&self) -> bool {
        if let ExtnType::Channel = self._type {
            if let ExtnClass::Device = self.class {
                return true;
            }
        }
        false
    }

    pub fn match_layer(&self, ref_cap: ExtnCapability) -> bool {
        if ref_cap._type == ExtnType::Main && self._type == ExtnType::Main {
            return true;
        }

        if self._type == ExtnType::Channel
            && ref_cap._type == self._type
            && ref_cap.class == self.class
        {
            return true;
        }

        if self._type == ExtnType::Extn
            && ref_cap.class == self.class
            && ref_cap.service == self.service
        {
            return true;
        }

        false
    }

    pub fn get_short(&self) -> String {
        if self._type == ExtnType::Channel {
            format!("{:?}:{:?}", self._type, self.class)
        } else {
            format!("{:?}:{:?}", self.class, self.service)
        }
    }

    pub fn get_type(&self) -> ExtnType {
        self._type.clone()
    }

    pub fn class(&self) -> ExtnClass {
        self.class.clone()
    }

    pub fn new_channel(class: ExtnClass, service: String) -> Self {
        Self {
            _type: ExtnType::Channel,
            class,
            service,
        }
    }

    pub fn new_extn(class: ExtnClass, service: String) -> Self {
        Self {
            _type: ExtnType::Extn,
            class,
            service,
        }
    }
}

impl PartialEq for ExtnCapability {
    fn eq(&self, other: &ExtnCapability) -> bool {
        self._type == other._type && self.class == other.class
    }
}

impl ExtnCapability {
    pub fn from(cap: String) -> Option<ExtnCapability> {
        let c_a = cap.split(":");
        if c_a.count() > 4 {
            let c_a: Vec<&str> = cap.split(":").collect();
            match c_a.get(0).unwrap().to_lowercase().as_str() {
                "ripple" => {}
                _ => return None,
            }
            let _type = ExtnType::get(c_a.get(1).unwrap());
            if _type.is_none() {
                return None;
            }
            let _type = _type.unwrap();

            let class = ExtnClass::get(c_a.get(2).unwrap());
            if class.is_none() {
                return None;
            }
            let class = class.unwrap();

            let service = c_a.get(3);
            if service.is_none() {
                return None;
            }
            let service = String::from(*service.unwrap());

            return Some(Self {
                _type,
                class,
                service,
            });
        }
        None
    }
}
