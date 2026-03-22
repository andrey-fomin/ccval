use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("unknown preset '{0}'")]
    UnknownPreset(String),
    #[error("failed to read config '{path}': {error}")]
    ReadFailed {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to parse config '{path}': {error}")]
    ParseFailed {
        path: String,
        #[source]
        error: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub preset: Option<String>,
    pub message: Option<FieldRules>,
    pub header: Option<FieldRules>,
    #[serde(rename = "type")]
    pub r#type: Option<FieldRules>,
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
    #[serde(default, skip_serializing_if = "Regexes::is_none")]
    pub regexes: Regexes,
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct Regexes(Option<Vec<regex_lite::Regex>>);

impl Regexes {
    pub fn is_none(regexes: &Self) -> bool {
        regexes.0.is_none()
    }

    pub fn as_ref(&self) -> Option<&[regex_lite::Regex]> {
        self.0.as_deref()
    }
}

impl From<Option<Vec<regex_lite::Regex>>> for Regexes {
    fn from(value: Option<Vec<regex_lite::Regex>>) -> Self {
        Self(value)
    }
}

impl serde::Serialize for Regexes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        match &self.0 {
            Some(regexes) => {
                let mut seq = serializer.serialize_seq(Some(regexes.len()))?;
                for regex in regexes {
                    seq.serialize_element(regex.as_str())?;
                }
                seq.end()
            }
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Regexes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let regexes = Option::<Vec<String>>::deserialize(deserializer)?;
        match regexes {
            Some(regexes) => {
                let mut compiled = Vec::with_capacity(regexes.len());
                for pattern in regexes {
                    match regex_lite::Regex::new(&pattern) {
                        Ok(regex) => compiled.push(regex),
                        Err(err) => {
                            return Err(serde::de::Error::custom(format!(
                                "invalid regex '{pattern}': {err}"
                            )));
                        }
                    }
                }
                Ok(Self(Some(compiled)))
            }
            None => Ok(Self(None)),
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
                regexes: o.regexes.clone().0.or_else(|| b.regexes.clone().0).into(),
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
            r#type: None,
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

    fn read_config_str(path: impl AsRef<Path>) -> Result<String, ConfigError> {
        let path = path.as_ref();
        std::fs::read_to_string(path).map_err(|error| ConfigError::ReadFailed {
            path: path.to_string_lossy().into_owned(),
            error,
        })
    }

    fn parse_config_str(path: impl AsRef<Path>, config_str: &str) -> Result<Config, ConfigError> {
        let path = path.as_ref();
        let path_string = path.to_string_lossy().into_owned();
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase);

        match extension.as_deref() {
            Some("toml") => toml::from_str(config_str).map_err(|error| ConfigError::ParseFailed {
                path: path_string,
                error: Box::new(error),
            }),
            Some("json") => {
                serde_json::from_str(config_str).map_err(|error| ConfigError::ParseFailed {
                    path: path_string,
                    error: Box::new(error),
                })
            }
            _ => serde_yaml::from_str(config_str).map_err(|error| ConfigError::ParseFailed {
                path: path_string,
                error: Box::new(error),
            }),
        }
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
            r#type: FieldRules::merge(base.r#type.as_ref(), overrides.r#type.as_ref()),
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

        serde_yaml::from_str(preset_yaml).map_err(|error| ConfigError::ParseFailed {
            path: format!("preset:{preset}"),
            error: Box::new(error),
        })
    }

    fn load_raw_from_str(
        path: impl AsRef<Path>,
        local_config_str: &str,
    ) -> Result<Config, ConfigError> {
        Self::parse_config_str(path, local_config_str)
    }

    fn load_raw_from_path(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
        let path = path.as_ref();
        let local_config_str = Self::read_config_str(path)?;
        Self::load_raw_from_str(path, &local_config_str)
    }

    fn apply_preset(local_config: Config, preset: Option<&str>) -> Result<Config, ConfigError> {
        let chosen_preset = preset
            .map(str::to_owned)
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
            None => Self::load_auto_discovered_config_in(Path::new("."))?,
        };

        Self::apply_preset(local_config, preset)
    }

    fn load_auto_discovered_config_in(base_dir: &Path) -> Result<Config, ConfigError> {
        let candidate_paths = [
            "conventional-commits.yaml",
            "conventional-commits.yml",
            "conventional-commits.toml",
            "conventional-commits.json",
        ];

        for path in candidate_paths {
            let full_path = base_dir.join(path);
            match full_path.try_exists() {
                Ok(true) => match full_path.metadata() {
                    Ok(metadata) => {
                        if metadata.is_file() {
                            return Self::load_raw_from_path(&full_path);
                        }

                        return Err(ConfigError::ReadFailed {
                            path: full_path.to_string_lossy().into_owned(),
                            error: std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "config path exists but is not a regular file",
                            ),
                        });
                    }
                    Err(error) => {
                        return Err(ConfigError::ReadFailed {
                            path: full_path.to_string_lossy().into_owned(),
                            error,
                        });
                    }
                },
                Ok(false) => {}
                Err(error) => {
                    return Err(ConfigError::ReadFailed {
                        path: full_path.to_string_lossy().into_owned(),
                        error,
                    });
                }
            }
        }

        Ok(Self::empty())
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
        let config = Config::apply_preset(
            Config::load_raw_from_str("config.yaml", custom_yaml).unwrap(),
            None,
        )
        .unwrap();

        assert_eq!(config.message.as_ref().unwrap().max_length, Some(1000));
        assert_eq!(config.message.as_ref().unwrap().max_line_length, None);
        assert!(config.r#type.is_none());
    }

    #[test]
    fn test_unknown_preset() {
        let custom_yaml = "
preset: unsupported
";
        let result = Config::apply_preset(
            Config::load_raw_from_str("config.yaml", custom_yaml).unwrap(),
            None,
        );
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

        let type_rules = merged.r#type.unwrap();
        assert_eq!(type_rules.values.unwrap(), vec!["docs"]); // overridden
    }

    #[test]
    fn test_merge_keeps_base_regexes_when_override_omits_regexes() {
        let base_yaml = "
description:
  regexes:
    - '^[^ ].*'
    - '^.*[^.]$'
";
        let override_yaml = "
description:
  max-length: 100
";
        let base: Config = serde_yaml::from_str(base_yaml).unwrap();
        let over: Config = serde_yaml::from_str(override_yaml).unwrap();

        let merged = Config::merge(&base, &over);
        let description_rules = merged.description.unwrap();

        assert_eq!(description_rules.max_length, Some(100));
        let regexes = description_rules.regexes.as_ref().unwrap();
        assert_eq!(regexes.len(), 2);
        assert_eq!(regexes[0].as_str(), "^[^ ].*");
        assert_eq!(regexes[1].as_str(), "^.*[^.]$");
    }

    #[test]
    fn test_regexes_round_trip_omitted_field_stays_omitted() {
        let rules: FieldRules = serde_yaml::from_str("max-length: 10\n").unwrap();

        assert!(Regexes::is_none(&rules.regexes));

        let yaml = serde_yaml::to_string(&rules).unwrap();
        assert!(!yaml.contains("regexes"));
    }

    #[test]
    fn test_regexes_round_trip_empty_field_stays_empty() {
        let rules: FieldRules = serde_yaml::from_str("regexes: []\n").unwrap();

        assert!(rules.regexes.0.as_ref().is_some_and(Vec::is_empty));

        let yaml = serde_yaml::to_string(&rules).unwrap();
        assert!(yaml.contains("regexes:"));
        assert!(yaml.contains("[]"));
    }

    #[test]
    fn test_invalid_yaml() {
        let invalid_yaml = "
header:
  max-length: not_a_number
";
        let result = Config::load_raw_from_str("config.yaml", invalid_yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_field() {
        let invalid_yaml = "
header:
  unknown: true
";
        let result = Config::load_raw_from_str("config.yaml", invalid_yaml);
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
        let result = Config::load_raw_from_str("config.yaml", invalid_yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid regex"));
    }

    #[test]
    fn test_auto_discovery_without_config_returns_empty_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = Config::load_auto_discovered_config_in(temp_dir.path()).unwrap();
        assert!(config.message.is_none());
        assert!(config.header.is_none());
        assert!(config.r#type.is_none());
    }

    #[test]
    fn test_load_from_path_requires_existing_file() {
        let result = Config::load_raw_from_path("definitely-missing-config.yaml");
        assert!(matches!(result, Err(ConfigError::ReadFailed { .. })));
    }

    #[test]
    fn test_load_raw_from_str_supports_toml() {
        let config = Config::load_raw_from_str(
            "config.toml",
            "[message]\nmax-line-length = 72\n\n[type]\nvalues = [\"feat\", \"fix\"]\n",
        )
        .unwrap();

        assert_eq!(config.message.as_ref().unwrap().max_line_length, Some(72));
        assert_eq!(
            config.r#type.as_ref().unwrap().values.as_ref().unwrap(),
            &vec!["feat".to_string(), "fix".to_string()]
        );
    }

    #[test]
    fn test_load_raw_from_str_supports_json() {
        let config = Config::load_raw_from_str(
            "config.JSON",
            r#"{"message":{"max-line-length":72},"type":{"values":["feat","fix"]}}"#,
        )
        .unwrap();

        assert_eq!(config.message.as_ref().unwrap().max_line_length, Some(72));
        assert_eq!(
            config.r#type.as_ref().unwrap().values.as_ref().unwrap(),
            &vec!["feat".to_string(), "fix".to_string()]
        );
    }

    #[test]
    fn test_auto_discovery_picks_first_config_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let yaml = temp_dir.path().join("conventional-commits.yaml");
        let toml = temp_dir.path().join("conventional-commits.toml");
        std::fs::write(&yaml, "message:\n  max-line-length: 10\n").unwrap();
        std::fs::write(&toml, "[message]\nmax-line-length = 20\n").unwrap();

        let config = Config::load_auto_discovered_config_in(temp_dir.path()).unwrap();
        assert_eq!(config.message.as_ref().unwrap().max_line_length, Some(10));
    }

    #[test]
    fn test_auto_discovery_errors_on_non_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp_dir.path().join("conventional-commits.yaml")).unwrap();

        let result = Config::load_auto_discovered_config_in(temp_dir.path());
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
