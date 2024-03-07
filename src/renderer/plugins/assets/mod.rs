mod image;
mod obj;

use bevy::prelude::*;

use crate::renderer::{model::Model, texture::Texture};

pub use self::obj::{ObjAssetsLoadState, ObjAssetsLoading};

pub struct AssetsPlugin;
impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(obj::ObjAssetsPlugin)
            .init_asset::<Model>()
            .init_asset::<Texture>()
            .add_systems(Startup, load_obj_models);
    }
}

fn load_obj_models(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ObjAssetsLoading>,
) {
    let monkey_handle: Handle<Model> = asset_server.load("monkey_smooth.obj");
    loading.0.insert("monkey".into(), monkey_handle);
}

/*
fn load_images(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ImageAssetsLoading>,
) {
    let backpack_handle:
}
*/
