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

  pub async fn get_user_by_email(&self, email: &str) -> Result<user::Model, DbErr> {
    user::Entity::find()
      .filter(user::Column::Email.eq(email.to_string()))
      .one(self.db)
      .await?
      .ok_or(DbErr::RecordNotFound(format!(
        "User with email {} not found",
        email
      )))
  }
}
