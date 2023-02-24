use crate::utils::error::RippleError;

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

/// Below is anatomy of a Ripple capability in a String format
///
/// `ripple:[channel/extn]:[class]:[service]:[feature (optional)]`

/// ## Decoding some ExtnCapabilit(ies)
///
/// Below capability means the given plugin offers a channel for Device Connection. Service used  for the device connection is Thunder.
///
/// `ripple:channel:device:thunder`
///
/// Below Capability means the given plugin offers a channel for Data Governance. Service used for the feature is `somegovernance` service
///
/// `ripple:channel:data-governance:somegovernance`
///
/// Below capability means the given plugin offers an extension for the Device Thunder Channel. It offers the auth thunder plugin implementation.
///
/// `ripple:extn:device:thunder:auth`
///
/// Below capability means the given plugin offers a permission extension for the distributor. Name of the service is called `fireboltpermissions`.
///
/// `ripple:extn:distributor:permissions:fireboltpermissions`
///
/// Below capability means the given plugin offers a JsonRpsee rpc extension for a service named badger
///
/// `ripple:extn:jsonrpsee:bridge`
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
    /// Returns the Main target capability for a given service
    /// # Arguments
    /// `service` - Type of String which defins the type of service
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_capability::ExtnCapability;
    ///
    /// let main_cap = ExtnCapability::get_main_target("cap".into());
    /// assert!(main_cap.to_string().eq("ripple:main:internal:cap"));
    /// ```
    pub fn get_main_target(service: String) -> ExtnCapability {
        ExtnCapability {
            _type: ExtnType::Main,
            class: ExtnClass::Internal,
            service,
        }
    }

    /// Checks if the given capability is a device channel.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_capability::{ExtnCapability,ExtnClass};
    ///
    /// let device_channel = ExtnCapability::new_channel(ExtnClass::Device, "info".into());
    /// assert!(device_channel.is_device_channel());
    /// ```
    pub fn is_device_channel(&self) -> bool {
        if let ExtnType::Channel = self._type {
            if let ExtnClass::Device = self.class {
                return true;
            }
        }
        false
    }

    /// Checks if the given capability has the same type and channel as the owner
    /// # Arguments
    /// `ref_cap` - Type of ExtnCapability which will be checked against the owner.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_capability::{ExtnCapability,ExtnClass};
    ///
    /// let info = ExtnCapability::new_channel(ExtnClass::Device, "info".into());
    /// let remote = ExtnCapability::new_channel(ExtnClass::Device, "remote".into());
    /// assert!(info.match_layer(remote));
    /// ```
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

    /// Gets a short form value of the given capability. Useful for ExtnClient to detect
    /// other senders for a given ExtnCapability.
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_capability::{ExtnCapability,ExtnClass};
    ///
    /// let device_channel = ExtnCapability::new_channel(ExtnClass::Device, "info".into());
    /// assert!(device_channel.get_short().eq("Channel:Device"));
    /// ```
    pub fn get_short(&self) -> String {
        if self._type == ExtnType::Channel {
            format!("{:?}:{:?}", self._type, self.class)
        } else {
            format!("{:?}:{}", self.class, self.service)
        }
    }

    /// Returns the `ExtnType` for the capability
    pub fn get_type(&self) -> ExtnType {
        self._type.clone()
    }

    /// Returns the `ExtnClass` for the capability
    pub fn class(&self) -> ExtnClass {
        self.class.clone()
    }

    /// Gets a new Channel capability based on the given `ExtnClass` and `Service`
    /// # Arguments
    /// `class` - Type of [ExtnClass]
    /// `service` - Type of String which defines a unique service for a given Class channel
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_capability::{ExtnCapability,ExtnClass};
    ///
    /// let device_channel = ExtnCapability::new_channel(ExtnClass::Device, "info".into());
    /// assert!(device_channel.get_short().eq("Channel:Device"));
    /// ```
    pub fn new_channel(class: ExtnClass, service: String) -> Self {
        Self {
            _type: ExtnType::Channel,
            class,
            service,
        }
    }

    /// Gets a new Channel capability based on the given `ExtnClass` and `Service`
    /// # Arguments
    /// `class` - Type of [ExtnClass]
    /// `service` - Type of String which defines a unique service for a given Class channel
    /// # Examples
    /// ```
    /// use ripple_sdk::extn::extn_capability::{ExtnCapability,ExtnClass};
    ///
    /// let device_channel = ExtnCapability::new_extn(ExtnClass::Device, "remote".into());
    /// assert!(device_channel.get_short().eq("Device:remote"));
    /// ```
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
