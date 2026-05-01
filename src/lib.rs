pub mod field {
    use serde::{Deserialize, Serialize};
    use std::borrow::Cow;
    use std::fmt::Display;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
    pub struct Name(Cow<'static, str>);

    impl Name {
        pub fn new<S: Into<String>>(s: S) -> Self {
            Self(Cow::from(s.into()))
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl Display for Name {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.as_str())
        }
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Value(Cow<'static, str>);

    impl Value {
        pub fn new<S: Into<String>>(s: S) -> Self {
            Self(Cow::from(s.into()))
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Field {
        name: Name,
        value: Value,
    }

    impl Field {
        pub fn new(name: Name, value: Value) -> Self {
            Self { name, value }
        }

        pub fn name(&self) -> &Name {
            &self.name
        }

        pub fn value(&self) -> &Value {
            &self.value
        }
    }
}

pub mod issue {
    use super::field;
    use serde::Deserialize;
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::convert::Infallible;
    use std::error::Error;
    use std::fmt::Display;
    use std::fs::File;
    use std::io::BufReader;
    use std::str::FromStr;

    #[derive(Debug, Deserialize, PartialEq, Eq, Hash, Clone)]
    pub struct Key(Cow<'static, str>);

    impl Key {
        pub fn new<S: Into<String>>(s: S) -> Self {
            Self(Cow::from(s.into()))
        }

        pub fn as_str(&self) -> &str {
            &self.0
        }

        pub fn into_string(self) -> String {
            self.0.into_owned()
        }
    }

    impl Display for Key {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.as_str())
        }
    }

    impl FromStr for Key {
        type Err = Infallible;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Self::new(s))
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Issue {
        fields: HashMap<field::Name, field::Value>,
    }

    impl Issue {
        pub fn field_value(&self, name: &field::Name) -> Option<&field::Value> {
            self.fields.get(name)
        }
    }

    #[derive(Debug, thiserror::Error)]
    #[error("could not get issue {key}")]
    pub struct GetIssueError {
        key: Key,
        source: Box<dyn Error + Send + Sync>,
    }

    pub fn get_issue(key: &Key) -> Result<Issue, GetIssueError> {
        let mut path = key.clone().into_string();
        path.push_str(".json");
        let file = File::open(&path).map_err(|e| GetIssueError {
            key: key.clone(),
            source: Box::new(e),
        })?;

        let reader = BufReader::new(file);

        serde_json::from_reader(reader).map_err(|e| GetIssueError {
            key: key.clone(),
            source: Box::new(e),
        })
    }
}
