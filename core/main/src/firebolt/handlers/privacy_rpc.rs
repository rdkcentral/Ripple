
pub const US_PRIVACY_KEY: &'static str = "us_privacy";
pub const LMT_KEY: &'static str = "lmt";

#[derive(Debug, Clone)]
struct AllowAppContentAdTargetingSettings {
    lmt: String,
    us_privacy: String,
}

impl AllowAppContentAdTargetingSettings {
    pub fn new(limit_ad_targeting: bool) -> Self {
        let (lmt, us_privacy) = match limit_ad_targeting {
            true => ("1", "1-Y-"),
            false => ("0", "1-N-"),
        };
        AllowAppContentAdTargetingSettings {
            lmt: lmt.to_owned(),
            us_privacy: us_privacy.to_owned(),
        }
    }

    pub fn get_allow_app_content_ad_targeting_settings(&self) -> HashMap<String, String> {
        HashMap::from([
            (US_PRIVACY_KEY.to_owned(), self.us_privacy.to_owned()),
            (LMT_KEY.to_owned(), self.lmt.to_owned()),
        ])
    }
}
impl Default for AllowAppContentAdTargetingSettings {
    fn default() -> Self {
        Self {
            /*
            As per X1 privacy settings documentation, default privacy setting is lmt = 0 us_privacy = 1-N-
            (i.e. , Customer has not opted-out)
            https://developer.comcast.com/documentation/limit-ad-tracking-and-ccpa-technical-requirements-1
            */
            lmt: "0".to_owned(),
            us_privacy: "1-N-".to_owned(),
        }
    }
}

pub async fn get_allow_app_content_ad_targeting_settings(
    platform_state: &PlatformState,
) -> HashMap<String, String> {
    let data = StorageProperty::AllowAppContentAdTargeting.as_data();

    match StorageManager::get_bool_from_namespace(
        platform_state,
        data.namespace.to_string(),
        data.key,
    )
    .await
    {
        Ok(resp) => AllowAppContentAdTargetingSettings::new(resp.as_value())
            .get_allow_app_content_ad_targeting_settings(),
        Err(StorageManagerError::NotFound) => AllowAppContentAdTargetingSettings::default()
            .get_allow_app_content_ad_targeting_settings(),
        _ => AllowAppContentAdTargetingSettings::new(true)
            .get_allow_app_content_ad_targeting_settings(),
    }
}