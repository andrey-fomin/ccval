use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("unknown preset '{0}'")]
    UnknownPreset(String),
    #[error("failed to read config '{path}': {error}")]
    ReadFailed { path: String, error: std::io::Error },
    #[error("{0}")]
    InvalidYaml(#[from] serde_yaml::Error),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub preset: Option<String>,
    pub message: Option<FieldRules>,
    pub header: Option<FieldRules>,
    #[serde(rename = "type")]
    pub commit_type: Option<FieldRules>,
    pub scope: Option<FieldRules>,
    pub description: Option<FieldRules>,
    pub body: Option<FieldRules>,
    pub footer_token: Option<FieldRules>,
    pub footer_value: Option<FieldRules>,
    pub footers: Option<HashMap<String, FieldRules>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FieldRules {
    pub max_length: Option<usize>,
    pub max_line_length: Option<usize>,
    pub required: Option<bool>,
    pub forbidden: Option<bool>,
    #[serde(with = "regexes_serde", default)]
    pub regexes: Option<Vec<regex_lite::Regex>>,
    pub values: Option<Vec<String>>,
}

mod regexes_serde {
    use regex_lite::Regex;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(regexes: &Option<Vec<Regex>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match regexes {
            Some(regexes) => serializer.collect_seq(regexes.iter().map(|regex| regex.as_str())),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<Regex>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<Vec<String>>::deserialize(deserializer)?;
        match opt {
            Some(regexes) => {
                let mut compiled = Vec::with_capacity(regexes.len());
                for pattern in regexes {
                    match Regex::new(&pattern) {
                        Ok(regex) => compiled.push(regex),
                        Err(err) => {
                            return Err(serde::de::Error::custom(format!(
                                "invalid regex '{}': {}",
                                pattern, err
                            )));
                        }
                    }
                }
                Ok(Some(compiled))
            }
            None => Ok(None),
        }
    }
}

impl FieldRules {
    pub fn merge(base: Option<&FieldRules>, overrides: Option<&FieldRules>) -> Option<FieldRules> {
        match (base, overrides) {
            (None, None) => None,
            (Some(b), None) => Some(b.clone()),
            (None, Some(o)) => Some(o.clone()),
            (Some(b), Some(o)) => Some(FieldRules {
                max_length: o.max_length.or(b.max_length),
                max_line_length: o.max_line_length.or(b.max_line_length),
                required: o.required.or(b.required),
                forbidden: o.forbidden.or(b.forbidden),
                regexes: o.regexes.clone().or_else(|| b.regexes.clone()),
                values: o.values.clone().or_else(|| b.values.clone()),
            }),
        }
    }
}

impl Config {
    fn empty() -> Config {
        Config {
            preset: None,
            message: None,
            header: None,
            commit_type: None,
            scope: None,
            description: None,
            body: None,
            footer_token: None,
            footer_value: None,
            footers: None,
        }
    }

    fn without_preset(mut config: Config) -> Config {
        config.preset = None;
        config
    }

    fn read_config_str(path: &str) -> Result<String, ConfigError> {
        std::fs::read_to_string(path).map_err(|error| ConfigError::ReadFailed {
            path: path.to_string(),
            error,
        })
    }

    pub fn merge(base: &Config, overrides: &Config) -> Config {
        let mut footers = HashMap::new();
        if let Some(b_footers) = &base.footers {
            for (k, v) in b_footers {
                footers.insert(k.clone(), v.clone());
            }
        }
        if let Some(o_footers) = &overrides.footers {
            for (k, v) in o_footers {
                let merged = FieldRules::merge(footers.get(k), Some(v)).unwrap();
                footers.insert(k.clone(), merged);
            }
        }

        Config {
            preset: overrides.preset.clone().or_else(|| base.preset.clone()),
            message: FieldRules::merge(base.message.as_ref(), overrides.message.as_ref()),
            header: FieldRules::merge(base.header.as_ref(), overrides.header.as_ref()),
            commit_type: FieldRules::merge(
                base.commit_type.as_ref(),
                overrides.commit_type.as_ref(),
            ),
            scope: FieldRules::merge(base.scope.as_ref(), overrides.scope.as_ref()),
            description: FieldRules::merge(
                base.description.as_ref(),
                overrides.description.as_ref(),
            ),
            body: FieldRules::merge(base.body.as_ref(), overrides.body.as_ref()),
            footer_token: FieldRules::merge(
                base.footer_token.as_ref(),
                overrides.footer_token.as_ref(),
            ),
            footer_value: FieldRules::merge(
                base.footer_value.as_ref(),
                overrides.footer_value.as_ref(),
            ),
            footers: if footers.is_empty() {
                None
            } else {
                Some(footers)
            },
        }
    }

    pub fn load_preset(preset: &str) -> Result<Config, ConfigError> {
        let preset_yaml = match preset {
            "default" => include_str!("default.yaml"),
            "strict" => include_str!("strict.yaml"),
            _ => return Err(ConfigError::UnknownPreset(preset.to_string())),
        };

        Ok(serde_yaml::from_str(preset_yaml)?)
    }

    fn load_raw_from_str(local_config_str: &str) -> Result<Config, ConfigError> {
        Ok(serde_yaml::from_str(local_config_str)?)
    }

    fn load_raw_default_path_if_exists(path: &str) -> Result<Config, ConfigError> {
        if !Path::new(path).exists() {
            return Ok(Self::empty());
        }

        let local_config_str = Self::read_config_str(path)?;
        Self::load_raw_from_str(&local_config_str)
    }

    fn load_raw_from_path(path: &str) -> Result<Config, ConfigError> {
        let local_config_str = Self::read_config_str(path)?;
        Self::load_raw_from_str(&local_config_str)
    }

    fn apply_preset(local_config: Config, preset: Option<&str>) -> Result<Config, ConfigError> {
        let chosen_preset = preset
            .map(|name| name.to_string())
            .or_else(|| local_config.preset.clone());
        let config_without_preset = Self::without_preset(local_config);

        match chosen_preset {
            Some(name) => {
                let preset_config = Self::load_preset(&name)?;
                let mut merged = Config::merge(&preset_config, &config_without_preset);
                merged.preset = Some(name);
                Ok(merged)
            }
            None => Ok(config_without_preset),
        }
    }

    pub fn load_with_preset(
        config_path: Option<&str>,
        preset: Option<&str>,
    ) -> Result<Config, ConfigError> {
        let local_config = match config_path {
            Some(path) => Self::load_raw_from_path(path)?,
            None => Self::load_raw_default_path_if_exists("conventional-commits.yaml")?,
        };

        Self::apply_preset(local_config, preset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preset_smoke() {
        Config::load_preset("default").unwrap();
    }

    #[test]
    fn test_strict_preset_smoke() {
        Config::load_preset("strict").unwrap();
    }

    #[test]
    fn test_load_from_str_without_preset_does_not_apply_defaults() {
        let custom_yaml = "
message:
  max-length: 1000
";
        let config =
            Config::apply_preset(Config::load_raw_from_str(custom_yaml).unwrap(), None).unwrap();

        assert_eq!(config.message.as_ref().unwrap().max_length, Some(1000));
        assert_eq!(config.message.as_ref().unwrap().max_line_length, None);
        assert!(config.commit_type.is_none());
    }

    #[test]
    fn test_unknown_preset() {
        let custom_yaml = "
preset: unsupported
";
        let result = Config::apply_preset(Config::load_raw_from_str(custom_yaml).unwrap(), None);
        assert!(
            matches!(result, Err(ConfigError::UnknownPreset(preset)) if preset == "unsupported")
        );
    }

    #[test]
    fn test_merge_configs() {
        let base_yaml = "
message:
  max-line-length: 100
type:
  values:
    - feat
    - fix
";
        let override_yaml = "
message:
  max-length: 500
type:
  values:
    - docs
";
        let base: Config = serde_yaml::from_str(base_yaml).unwrap();
        let over: Config = serde_yaml::from_str(override_yaml).unwrap();

        let merged = Config::merge(&base, &over);

        let msg_rules = merged.message.unwrap();
        assert_eq!(msg_rules.max_line_length, Some(100)); // inherited
        assert_eq!(msg_rules.max_length, Some(500)); // overridden

        let type_rules = merged.commit_type.unwrap();
        assert_eq!(type_rules.values.unwrap(), vec!["docs"]); // overridden
    }

    #[test]
    fn test_invalid_yaml() {
        let invalid_yaml = "
header:
  max-length: not_a_number
";
        let result = Config::load_raw_from_str(invalid_yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_field() {
        let invalid_yaml = "
header:
  unknown: true
";
        let result = Config::load_raw_from_str(invalid_yaml);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown field `unknown`")
        );
    }

    #[test]
    fn test_invalid_regex() {
        let invalid_yaml = r#"
header:
  regexes:
    - "(unclosed"
"#;
        let result = Config::load_raw_from_str(invalid_yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid regex"));
    }

    #[test]
    fn test_load_default_path_if_missing_returns_empty_config() {
        let config =
            Config::load_raw_default_path_if_exists("definitely-missing-config.yaml").unwrap();
        assert!(config.message.is_none());
        assert!(config.header.is_none());
        assert!(config.commit_type.is_none());
    }

    #[test]
    fn test_load_from_path_requires_existing_file() {
        let result = Config::load_raw_from_path("definitely-missing-config.yaml");
        assert!(matches!(result, Err(ConfigError::ReadFailed { .. })));
    }

    #[test]
    fn test_load_with_preset_overrides_config_file_preset() {
        let config_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            config_file.path(),
            "preset: default\nmessage:\n  max-length: 200\n",
        )
        .unwrap();

        let config =
            Config::load_with_preset(Some(config_file.path().to_str().unwrap()), Some("strict"))
                .unwrap();
        assert_eq!(config.preset.as_deref(), Some("strict"));
        assert!(config.header.is_some());
        assert_eq!(
            config.message.as_ref().and_then(|m| m.max_length),
            Some(200)
        );
    }
}
