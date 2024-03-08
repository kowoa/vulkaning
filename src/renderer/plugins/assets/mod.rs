mod image;
mod obj;

use bevy::prelude::*;

use crate::renderer::{model::Model, texture::Texture};

pub use self::{
    image::{ImageAssetsLoadState, ImageAssetsLoading},
    obj::{ObjAssetsLoadState, ObjAssetsLoading},
};

pub struct AssetsPlugin;
impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((obj::ObjAssetsPlugin, image::ImageAssetsPlugin))
            .init_asset::<Model>()
            .init_asset::<Texture>()
            .add_systems(PreStartup, (load_obj_models, load_image_textures));
    }
}

fn load_obj_models(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ObjAssetsLoading>,
) {
    let monkey = asset_server.load("monkey_smooth.obj");
    loading.0.insert("monkey".into(), monkey);

    let backpack = asset_server.load("backpack/backpack.obj");
    loading.0.insert("backpack".into(), backpack);

    let empire = asset_server.load("lost_empire.obj");
    loading.0.insert("empire".into(), empire);
}

fn load_image_textures(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ImageAssetsLoading>,
) {
    let backpack = asset_server.load("backpack/diffuse.jpg");
    loading.0.insert("backpack".into(), backpack);
}
