use entity::user;
use sea_orm::{IntoActiveModel, prelude::*};

pub struct UserTable<'db> {
  db: &'db DatabaseConnection,
}

impl<'db> UserTable<'db> {
  pub fn new(db: &'db DatabaseConnection) -> Self {
    Self { db }
  }

  pub async fn create_user(
    &self,
    username: String,
    email: String,
    password: String,
    salt: String,
  ) -> Result<Uuid, DbErr> {
    let model = user::Model {
      id: Uuid::new_v4(),
      name: username,
      email,
      password,
      salt,
    }
    .into_active_model();

    let ret = model.insert(self.db).await?;

    Ok(ret.id)
  }
}
