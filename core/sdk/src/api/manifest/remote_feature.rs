use crate::{
    api::config::Config,
    extn::{client::extn_client::ExtnClient, extn_client_message::ExtnResponse},
};
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn val_to_bool_parse(val: Value, def: bool) -> bool {
    if val.is_boolean() {
        return val.as_bool().unwrap_or(def);
    }
    if let Some(s) = val.as_str() {
        if let Ok(sb) = s.parse() {
            return sb;
        }
    }
    def
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FeatureFlag {
    pub default: bool,
    pub remote_key: Option<String>,
}

pub struct RemoteFeature {}

impl RemoteFeature {
    pub async fn flag(extn_client: &mut ExtnClient, flag_cfg: FeatureFlag) -> bool {
        if flag_cfg.remote_key.is_none() {
            return flag_cfg.default;
        }
        let key = flag_cfg.remote_key.unwrap();
        let rfc_res = extn_client.request(Config::RFC(key.clone())).await;
        let mut val = flag_cfg.default;
        if let Ok(resp) = rfc_res {
            if let Some(ExtnResponse::Value(v)) = resp.payload.extract::<ExtnResponse>() {
                info!("RFC Value for {} = {}", key, v);
                val = val_to_bool_parse(v, flag_cfg.default);
            } else {
                error!("Invalid RFC flag {}", key);
            }
        } else {
            error!("Failure to retrieve RFC flag {}", key)
        }
        val
    }
}

#[cfg(test)]
pub mod tests {
    use serde_json::Value;
    use tokio::spawn;

    use crate::{
        api::{
            config::Config,
            manifest::remote_feature::{FeatureFlag, RemoteFeature},
        },
        create_processor,
        extn::{
            client::extn_processor::ExtnStreamer,
            extn_client_message::{ExtnMessage, ExtnResponse},
            mock_extension_client::MockExtnClient,
        },
        utils::error::RippleError,
    };

    create_processor!(MockRFC, Config);

    #[tokio::test]
    pub async fn test_flag_no_remote() {
        let ff = FeatureFlag {
            default: false,
            remote_key: None,
        };
        let mut main = MockExtnClient::main();
        assert!(!RemoteFeature::flag(&mut main, ff).await);
    }

    #[tokio::test]
    pub async fn test_flag_with_remote_as_true() {
        test_flag_with_remote("myflag", Value::String(String::from("true")), true).await;
    }

    #[tokio::test]
    pub async fn test_flag_with_remote_as_false() {
        test_flag_with_remote("myflag", Value::String(String::from("false")), false).await;
    }

    #[tokio::test]
    pub async fn test_flag_with_remote_as_true_bool() {
        test_flag_with_remote("myflag", Value::Bool(true), true).await;
    }

    #[tokio::test]
    pub async fn test_flag_with_remote_not_set() {
        test_flag_with_remote("unknown", Value::Null, false).await;
    }

    pub async fn test_flag_with_remote(rfc_key: &str, rfc_val: Value, flag: bool) {
        let ff = FeatureFlag {
            default: false,
            remote_key: Some(String::from(rfc_key)),
        };

        let (mut client, _, mut rcv) = MockRFC::mock_extn_client();
        let mut client_for_mock = client.clone();
        spawn(async move {
            let (msg, rfc_req) = rcv.recv().await.unwrap().as_msg().unwrap();
            if let Config::RFC(rfc) = rfc_req {
                if rfc == "myflag" {
                    let response = ExtnResponse::Value(rfc_val);
                    let resp = msg.get_response(response);
                    client_for_mock.send_message(resp.unwrap()).await.ok();
                    return;
                }
                let response = ExtnResponse::Error(RippleError::InvalidInput);
                let resp = msg.get_response(response);
                client_for_mock.send_message(resp.unwrap()).await.ok();
            }
        });
        assert_eq!(RemoteFeature::flag(&mut client, ff).await, flag);
    }
}
