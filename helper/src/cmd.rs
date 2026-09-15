//! JSON-line commands from QML. Wire names stay strings; this enum is the gate.

use crate::error::{Error, Result};
use crate::params::{self, ParamId, SetPayload};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmd {
    Select,
    Tab { group: &'static str },
    GetAll,
    Refresh,
    Set { param: ParamId, payload: SetPayload },
    AfTarget,
    AfArea,
    Heartbeat,
    Preview { on: bool },
    Reconnect,
}

impl Cmd {
    pub fn parse(v: &serde_json::Value) -> Result<Self> {
        let cmd = v.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        match cmd {
            "select" => Ok(Self::Select),
            "tab" => {
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("exposure");
                Ok(Self::Tab {
                    group: params::tab_group(name),
                })
            }
            "get-all" => Ok(Self::GetAll),
            "refresh" => Ok(Self::Refresh),
            "set" => {
                let name = v
                    .get("name")
                    .and_then(|x| x.as_str())
                    .ok_or_else(|| Error::Invalid("name".into()))?;
                let param = ParamId::parse(name)
                    .ok_or_else(|| Error::Invalid(format!("unknown {name}")))?;
                let payload = params::parse_set(param.spec(), v)?;
                Ok(Self::Set { param, payload })
            }
            "af-target" => Ok(Self::AfTarget),
            "af-area" => Ok(Self::AfArea),
            "heartbeat" => Ok(Self::Heartbeat),
            "preview" => {
                let on = v
                    .get("on")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                Ok(Self::Preview { on })
            }
            "reconnect" => Ok(Self::Reconnect),
            _ => Err(Error::Invalid(format!("unknown cmd {cmd}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cmd;
    use crate::params::{ParamId, SetPayload};
    use serde_json::json;

    #[test]
    fn parse_unknown_cmd_fails() {
        assert!(Cmd::parse(&json!({"cmd": "explode"})).is_err());
        assert!(Cmd::parse(&json!({})).is_err());
    }

    #[test]
    fn parse_unknown_param_fails() {
        let err =
            Cmd::parse(&json!({"cmd": "set", "name": "color-style", "value": "1"})).unwrap_err();
        assert!(err.to_string().contains("unknown color-style"));
        assert!(Cmd::parse(&json!({"cmd": "set", "name": "focus-enhance", "value": "1"})).is_err());
    }

    #[test]
    fn parse_set_iso_from_qml() {
        let cmd = Cmd::parse(&json!({"cmd": "set", "name": "iso", "value": "400"})).unwrap();
        assert_eq!(
            cmd,
            Cmd::Set {
                param: ParamId::Iso,
                payload: SetPayload::Int(400)
            }
        );
    }

    #[test]
    fn parse_set_rejects_int_on_wb_and_getonly() {
        assert!(Cmd::parse(&json!({"cmd": "set", "name": "wb-offset", "value": "10"})).is_err());
        assert!(Cmd::parse(&json!({"cmd": "set", "name": "firmware", "value": "1"})).is_err());
        let wb = Cmd::parse(&json!({"cmd": "set", "name": "wb-offset", "r": 8, "b": 12})).unwrap();
        assert_eq!(
            wb,
            Cmd::Set {
                param: ParamId::WbOffset,
                payload: SetPayload::Wb { r: 8, b: 12 }
            }
        );
    }

    #[test]
    fn parse_tab_and_preview() {
        assert_eq!(
            Cmd::parse(&json!({"cmd": "tab", "name": "image"})).unwrap(),
            Cmd::Tab { group: "image" }
        );
        assert_eq!(
            Cmd::parse(&json!({"cmd": "preview", "on": true})).unwrap(),
            Cmd::Preview { on: true }
        );
        assert_eq!(
            Cmd::parse(&json!({"cmd": "refresh"})).unwrap(),
            Cmd::Refresh
        );
    }
}
