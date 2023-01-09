use crate::schema::group_user;
use crate::schema::groups;
use super::database_connection::Database;
use crate::models::group::*;
use crate::models::group_user::*;
use crate::models::user::*;
use crate::service::group_service::GroupService;
use diesel::prelude::*;
pub struct UserService {
    pub conn: diesel::PgConnection,
}
impl UserService {
    pub fn new() -> UserService {
        UserService {
            conn: (Database::connect()),
        }
    }
    pub fn create_user(&mut self, username: &String) -> Result<User, ()> {
        match self.get_user_by_name(username) {
            None => {}
            Some(..) => {
                println!("User with this name already exists!");
                return Err(());
            }
        }
        let new_user = NewUser::new(String::clone(username));
        use crate::schema::users::dsl::*;
        let user = diesel::insert_into(users)
            .values(&new_user)
            .get_result::<User>(&mut self.conn);
        match user {
            Ok(u) => {
                println!("Created user with id: {}, name: {}", u.id, u.name);
                Ok(u)
            }
            Err(..) => {
                println!("Error occured putting user {} in database", username);
                Err(())
            }
        }
    }
    fn get_user_by_name(&mut self, username: &String) -> Option<User> {
        use crate::schema::users::dsl::*;
        let user = users
            .filter(ExpressionMethods::eq(name, username))
            .first::<User>(&mut self.conn);
        match user {
            Ok(u) => Some(u),
            Err(..) => None,
        }
    }
    pub fn create_group(&mut self, caller_name: &String, group_name: &String) -> Result<Group, ()> {
        if !Self::is_user_in_database(&caller_name) {
            return Err(());
        }
        match GroupService::new().get_group_by_name(group_name) {
            None => {}
            Some(..) => {
                println!("Group with this name already exists!");
                return Err(());
            }
        }
        let caller = self.get_user_by_name(caller_name).unwrap();
        let new_group = NewGroup::new(String::clone(&group_name));
        let group = diesel::insert_into(groups::table)
            .values(&new_group)
            .get_result::<Group>(&mut self.conn);
        let group = group.unwrap();
        println!(
            "Created group with id: {}, name: {}, status: {}",
            group.id, group.name, group.status
        );
        Self::set_first_admin(&caller, &group, &mut self.conn);
        Ok(group)
    }
    fn set_first_admin(user: &User, group: &Group, conn: &mut PgConnection) {
        Self::create_group_user_link(group, user, UserRole::Admin, conn);
    }
    fn create_group_user_link(
        group: &Group,
        new_member: &User,
        role: UserRole,
        conn: &mut PgConnection,
    ) -> Result<(), ()> {
        let new_group_user = NewGroupUser::new(group.id, new_member.id, role, None);
        if Self::is_user_in_group(&new_member, &group, conn) {
            println!("User {} is already in {}", new_member.name, group.name);
            return Err(());
        }
        let group_user = diesel::insert_into(group_user::table)
            .values(&new_group_user)
            .get_result::<GroupUser>(conn);
        let group_user = group_user.unwrap();
        println!(
            "Created group_user link: group: {}, user: {}, role: {}",
            group.name, new_member.name, group_user.role
        );
        Ok(())
    }
    fn is_user_in_database(user_name: &String) -> bool {
        match UserService::new().get_user_by_name(&user_name) {
            None => {
                println!("User with name {} was not found", user_name);
                false
            }
            Some(..) => true,
        }
    }
    fn is_user_in_group(user: &User, group: &Group, conn: &mut PgConnection) -> bool {
        use crate::schema::group_user::dsl::*;
        let user = group_user
            .filter(BoolExpressionMethods::and(
                ExpressionMethods::eq(group_id, group.id),
                ExpressionMethods::eq(user_id, user.id),
            ))
            .first::<GroupUser>(conn);
        match user {
            Ok(..) => true,
            Err(..) => false,
        }
    }
    pub fn join_group(&mut self, caller_name: &String, group_name: &String) -> Result<(), ()> {
        if !Self::is_user_in_database(&caller_name) {
            return Err(());
        }
        let mut caller = self.get_user_by_name(caller_name).unwrap();
        let mut group_service = GroupService::new();
        let group = match group_service.get_group_by_name(&group_name) {
            None => {
                println!("Group with this name does not exist!");
                return Err(());
            }
            Some(g) => g,
        };
        if group.status == GroupStatus::Closed {
            println!("Group with name {} is closed", group.name);
            return Err(());
        }
        Self::create_group_user_link(&group, &caller, UserRole::User, &mut self.conn);
        Ok(())
    }
    fn is_group_in_database(group_name: &String) -> bool {
        match GroupService::new().get_group_by_name(&group_name) {
            None => false,
            Some(..) => true,
        }
    }
    pub fn close_group(&mut self, caller: &User, group: &Group) -> Result<(), ()> {
        use crate::schema::groups::dsl::*;
        if !Self::is_user_in_database(&caller.name) {
            println!("User {} was not found in database", caller.name);
            return Err(());
        }
        if !Self::is_group_in_database(&group.name) {
            println!("Group {} was not found in database", group.name);
            return Err(());
        }
        if !Self::is_user_in_group(caller, group, &mut self.conn) {
            println!("User {} was not found in group {}", caller.name, group.name);
            return Err(());
        }
        if !Self::is_admin(caller, group, &mut self.conn) {
            println!("User {} is not admin in group {}", caller.name, group.name);
            return Err(());
        }
        diesel::update(groups.filter(id.eq(group.id)))
            .set(current_state.eq(GroupStatus::Closed))
            .get_result::<Group>(&mut self.conn);
        Ok(())
    }
    fn is_admin(user: &User, group: &Group, conn: &mut PgConnection) -> bool {
        use crate::schema::group_user::dsl::*;
        let user = group_user
            .filter(BoolExpressionMethods::and(
                BoolExpressionMethods::and(group_id.eq(group.id), user_id.eq(user.id)),
                user_role.eq(UserRole::Admin),
            ))
            .first::<GroupUser>(conn);
        match user {
            Ok(..) => true,
            Err(..) => false,
        }
    }
}