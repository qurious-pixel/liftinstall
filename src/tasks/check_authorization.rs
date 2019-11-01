
use installer::InstallerFramework;
use tasks::{Task, TaskDependency, TaskMessage, TaskOrdering, TaskParamType};
use logging::LoggingErrors;
use frontend::rest::services::authentication;
use futures::{Stream, Future};
use tasks::resolver::ResolvePackageTask;

pub struct CheckAuthorizationTask {
    pub name: String,
}

impl Task for CheckAuthorizationTask {
    fn execute(
        &mut self,
        mut input: Vec<TaskParamType>,
        context: &mut InstallerFramework,
        messenger: &dyn Fn(&TaskMessage),
    ) -> Result<TaskParamType, String> {

        assert_eq!(input.len(), 1);
        let params = input.pop().log_expect("Should have input from resolver!");
        let (version, file) = match params {
            TaskParamType::File(v, f) => { Ok((v, f)) },
            _ => { Err("Unexpected TaskParamType in CheckAuthorization: {:?}") }
        }?;

        if !file.requires_authorization {
            return Ok(TaskParamType::Authentication(version, file, None));
        }

        let username = context.database.credentials.username.clone();
        let token = context.database.credentials.token.clone();
        let authentication = context.config.clone().unwrap().authentication.unwrap();
        let auth_url = authentication.auth_url.clone();
        let pub_key_base64 = authentication.pub_key_base64.clone();
        let validation = authentication.validation.clone();
        // Authorizaion is required for this package so post the username and token and get a jwt_token response
        let jwt_token = match authentication::authenticate_sync(auth_url, username, token) {
            Ok(jwt) => jwt,
            Err(_) => return Ok(TaskParamType::Authentication(version, file, None))
        };
        let claims = match authentication::validate_token(jwt_token.clone(), pub_key_base64, validation) {
            Ok(c) => c,
            Err(_) => return Ok(TaskParamType::Authentication(version, file, None))
        };
        // Validate that they are authorized
        let authorized =
                claims.roles.contains(&"vip".to_string()) || (claims.channels.contains(&"early-access".to_string()));

        if !authorized {
            return Ok(TaskParamType::Authentication(version, file, None));
        }
        Ok(TaskParamType::Authentication(version, file, Some(jwt_token)))
    }

    fn dependencies(&self) -> Vec<TaskDependency> {
        vec![TaskDependency::build(
            TaskOrdering::Pre,
            Box::new(ResolvePackageTask {
                name: self.name.clone(),
            }),
        )]
    }

    fn name(&self) -> String {
        format!("CheckAuthorizationTask (for {:?})", self.name)
    }
}