use entity::user;
use sea_orm::{IntoActiveModel, Set, prelude::*};

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
  ) -> centaurus::error::Result<Uuid> {
    let url = crate::gravatar::get_gravatar_url(&email);
    let data = match reqwest::get(&url).await {
      Ok(response) => match response.bytes().await {
        Ok(bytes) => Some(hex::encode(bytes)),
        Err(_) => None,
      },
      Err(_) => None,
    };

    let model = user::Model {
      id: Uuid::new_v4(),
      name: username,
      email,
      password,
      salt,
      avatar: data,
    }
    .into_active_model();

    let ret = model.insert(self.db).await?;

    Ok(ret.id)
  }

  pub async fn try_get_user_by_email(&self, email: &str) -> Result<Option<user::Model>, DbErr> {
    user::Entity::find()
      .filter(user::Column::Email.eq(email.to_string()))
      .one(self.db)
      .await
  }

  pub async fn get_user_by_email(&self, email: &str) -> Result<user::Model, DbErr> {
    self
      .try_get_user_by_email(email)
      .await?
      .ok_or(DbErr::RecordNotFound(format!(
        "User with email {} not found",
        email
      )))
  }

  pub async fn get_user_by_id(&self, id: Uuid) -> Result<user::Model, DbErr> {
    user::Entity::find_by_id(id)
      .one(self.db)
      .await?
      .ok_or(DbErr::RecordNotFound(format!(
        "User with id {} not found",
        id
      )))
  }

  pub async fn update_user_password(&self, id: Uuid, new_password: String) -> Result<(), DbErr> {
    let mut user: user::ActiveModel = self.get_user_by_id(id).await?.into();

    user.password = Set(new_password);

    user.update(self.db).await?;

    Ok(())
  }

  pub async fn update_user_name(&self, id: Uuid, new_name: String) -> Result<(), DbErr> {
    let mut user: user::ActiveModel = self.get_user_by_id(id).await?.into();

    user.name = Set(new_name);

    user.update(self.db).await?;

    Ok(())
  }

  pub async fn update_user_avatar(&self, id: Uuid, new_avatar: String) -> Result<(), DbErr> {
    let mut user: user::ActiveModel = self.get_user_by_id(id).await?.into();

    user.avatar = Set(Some(new_avatar));

    user.update(self.db).await?;

    Ok(())
  }
}
