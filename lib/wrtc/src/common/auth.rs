use crate::scanf;
use indexmap::IndexMap;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("token is not correct.")]
    TokenIsNotCorrect,

    #[error("no token found.")]
    NoTokenFound,

    #[error("invalid token format.")]
    InvalidTokenFormat,
}

pub enum SecretCarrier {
    Query(String),
    Bearer(String),
}

pub fn get_secret(carrier: &SecretCarrier) -> Result<String, AuthError> {
    match carrier {
        SecretCarrier::Query(query) => {
            let mut query_pairs = IndexMap::new();
            let pars_array: Vec<&str> = query.split('&').collect();
            for ele in pars_array {
                let (k, v) = scanf!(ele, '=', String, String);
                if k.is_none() || v.is_none() {
                    continue;
                }
                query_pairs.insert(k.unwrap(), v.unwrap());
            }

            query_pairs
                .get("token")
                .map_or(Err(AuthError::NoTokenFound), |t| Ok(t.to_string()))
        }
        SecretCarrier::Bearer(header) => {
            let invalid_format = Err(AuthError::InvalidTokenFormat);
            let (prefix, token) = scanf!(header, " ", String, String);

            match token {
                Some(token_val) => match prefix {
                    Some(prefix_val) => {
                        if prefix_val != "Bearer" {
                            invalid_format
                        } else {
                            Ok(token_val)
                        }
                    }
                    None => invalid_format,
                },
                None => invalid_format,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Auth {
    token: String,
}

impl Auth {
    pub fn new(token: String) -> Self {
        Self { token }
    }

    pub fn authenticate(&self, secret: &Option<SecretCarrier>) -> Result<(), AuthError> {
        let mut auth_err_reason: String = String::from("there is no token str found.");
        let mut err = AuthError::NoTokenFound;

        if let Some(secret_value) = secret {
            let token = get_secret(secret_value)?;
            if self.check(token.as_str()) {
                return Ok(());
            }
            auth_err_reason = format!("token is not correct: {token}");
            err = AuthError::TokenIsNotCorrect;
        }

        log::error!("Auth error stream_name: {auth_err_reason}, reason: {auth_err_reason}",);
        return Err(err);
    }

    fn check(&self, token: &str) -> bool {
        self.token == token
    }
}
