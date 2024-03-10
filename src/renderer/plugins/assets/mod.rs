mod image;
mod obj;

use bevy::prelude::*;

use crate::renderer::texture::TextureAssetData;

use self::{image::ImageAssetData, obj::ObjAssetData};
pub use self::{
    image::{ImageAssetsLoadState, ImageAssetsLoading},
    obj::{ObjAssetsLoadState, ObjAssetsLoading},
};

pub struct AssetsPlugin;
impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((obj::ObjAssetsPlugin, image::ImageAssetsPlugin))
            .init_asset::<ObjAssetData>()
            .init_asset::<ImageAssetData>()
            .add_systems(PreStartup, (load_obj_assets, load_image_assets));
    }
}

fn load_obj_assets(
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

fn load_image_assets(
    asset_server: Res<AssetServer>,
    mut loading: ResMut<ImageAssetsLoading>,
) {
    let backpack = asset_server.load("backpack/diffuse.jpg");
    loading.0.insert(
        "backpack".into(),
        (
            backpack,
            TextureAssetData {
                data: None,
                flipv: true,
                filter: ash::vk::Filter::LINEAR,
            },
        ),
    );

    let empire = asset_server.load("lost_empire-RGBA.png");
    loading
        .0
        .insert("empire".into(), (empire, TextureAssetData::default()));
}
