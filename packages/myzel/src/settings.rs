use nodarium_utils::{
    concat_arg_vecs, decode_float, encode_float,
    geometry::{is_path, path_link, split_items},
    split_args, wrap_arg,
};

use crate::paths::{MeshPath, PathGroup};

pub const SETTING_TAG: i32 = 13;

/// A value of `hyphae_to_mesh` that the paths of some nodes bring with them
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// How far, in path radii, neighbouring paths join into one surface
    Blend = 0,
    /// Strength of the fibrous relief
    Fibres = 1,
    /// Where a ball of this many path radii can't pass between body paths, the gap fills
    Fill = 2,
}

impl Setting {
    fn from_index(index: i32) -> Option<Setting> {
        match index {
            0 => Some(Setting::Blend),
            1 => Some(Setting::Fibres),
            2 => Some(Setting::Fill),
            _ => None,
        }
    }
}

/// What the paths bring with them, `None` keeps the value of `hyphae_to_mesh`
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PathSettings {
    pub blend: Option<f32>,
    pub fibres: Option<f32>,
    pub fill: Option<f32>,
}

impl PathSettings {
    fn set(&mut self, setting: Setting, value: f32) {
        let slot = match setting {
            Setting::Blend => &mut self.blend,
            Setting::Fibres => &mut self.fibres,
            Setting::Fill => &mut self.fill,
        };
        *slot = Some(value);
    }
}

/// A setting for the paths of some nodes `[13, setting, value, ...creators]`, see [`Setting`].
/// It travels in a list next to the paths, like the geometry an `instance` node adds.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingItem {
    pub setting: Setting,
    pub value: f32,
    /// The nodes whose paths get the value
    pub creators: Vec<i32>,
}

impl SettingItem {
    pub fn encode(&self) -> Vec<i32> {
        let mut words = vec![SETTING_TAG, self.setting as i32, encode_float(self.value)];
        words.extend(&self.creators);
        words
    }

    /// The item of a list item, brackets included, `None` for any other item
    pub fn decode(item: &[i32]) -> Option<SettingItem> {
        let n = item.len();
        // one group of plain values: its count points at its own closing bracket
        if n < 7 || item[0] != 0 || item[1] as usize != n - 3 || item[2] != SETTING_TAG {
            return None;
        }
        let words = &item[2..n - 2];
        let setting = Setting::from_index(words[1])?;
        Some(SettingItem { setting, value: decode_float(words[2]), creators: words[3..].to_vec() })
    }
}

/// A list inside a list starts with a group, its count then points at index 2. A path never has
/// a count of 1, it counts its header and points.
fn is_list(item: &[i32]) -> bool {
    item.len() > 4 && item[0] == 0 && item[1] == 1
}

fn collect_creators(list: &[i32], out: &mut Vec<i32>) {
    for item in split_args(list) {
        if is_list(item) {
            collect_creators(item, out);
        } else if is_path(item) {
            let creator = path_link(item).creator;
            if !out.contains(&creator) {
                out.push(creator);
            }
        }
    }
}

/// The list with a setting for the nodes whose paths are in it, also those in lists inside it.
/// Everything in the list stays as it is, a list without paths gets no setting.
pub fn with_setting(list: &[i32], setting: Setting, value: f32) -> Vec<i32> {
    let mut creators = Vec::new();
    if list.len() >= 4 && list[0] == 0 {
        collect_creators(list, &mut creators);
    }
    let mut items: Vec<Vec<i32>> = split_items(list).iter().map(|item| item.to_vec()).collect();
    if !creators.is_empty() {
        items.push(wrap_arg(&SettingItem { setting, value, creators }.encode()));
    }
    concat_arg_vecs(items)
}

fn collect_settings(list: &[i32], out: &mut Vec<SettingItem>) {
    for item in split_args(list) {
        if let Some(setting) = SettingItem::decode(item) {
            out.push(setting);
        } else if is_list(item) {
            collect_settings(item, out);
        }
    }
}

/// Every setting of a list, also those in lists inside it, in their order
pub fn read_settings(list: &[i32]) -> Vec<SettingItem> {
    let mut out = Vec::new();
    if list.len() >= 4 && list[0] == 0 {
        collect_settings(list, &mut out);
    }
    out
}

/// Gives every path made by a node the settings for that node. A later setting replaces an
/// earlier one, so a node closer to the mesh wins. Grown hyphae have no creator and get none.
pub fn apply_settings(paths: &mut [MeshPath], settings: &[SettingItem]) {
    for path in paths {
        let PathGroup::Creator(creator) = path.group else { continue };
        for s in settings.iter().filter(|s| s.creators.contains(&creator)) {
            path.settings.set(s.setting, s.value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{read_paths, PathStyle};
    use nodarium_utils::geometry::{create_multiple_paths, wrap_multiple_paths};

    fn list(amount: usize, creator: i32) -> Vec<i32> {
        let mut paths = create_multiple_paths(amount, 3, 1, creator);
        for (k, path) in wrap_multiple_paths(&mut paths).iter_mut().enumerate() {
            for i in 0..3 {
                path.points[i * 4..i * 4 + 4].copy_from_slice(&[k as f32, i as f32, 0.0, 0.1]);
            }
        }
        paths
    }

    #[test]
    fn a_setting_travels_next_to_the_paths() {
        let paths = concat_arg_vecs(vec![list(2, 5), list(1, 9)]);
        let set = with_setting(&paths, Setting::Fibres, 1.5);
        // the paths stay as they are and the setting names both of their nodes
        assert_eq!(read_paths(&set, PathStyle::BODY), read_paths(&paths, PathStyle::BODY));
        let settings = read_settings(&set);
        assert_eq!(settings, vec![SettingItem { setting: Setting::Fibres, value: 1.5, creators: vec![5, 9] }]);
        assert!(read_settings(&paths).is_empty());
        // through a join, which takes the items of its inputs into one list
        let joined = concat_arg_vecs(split_args(&set).iter().map(|i| i.to_vec()).chain([wrap_arg(&list(1, 2))]).collect());
        assert_eq!(read_settings(&joined), settings);
        // a list without paths gets nothing, an unconnected input stays empty
        assert_eq!(with_setting(&[0, 1, 1, 1], Setting::Blend, 2.0), vec![0, 1, 1, 1]);
    }

    #[test]
    fn settings_are_found_in_lists_inside_lists() {
        let inner = with_setting(&list(1, 4), Setting::Blend, 2.0);
        let nested = concat_arg_vecs(vec![inner, list(1, 6)]);
        let set = with_setting(&nested, Setting::Fill, 3.0);
        let found = read_settings(&set);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].setting, found[0].creators.clone()), (Setting::Blend, vec![4]));
        assert_eq!((found[1].setting, found[1].creators.clone()), (Setting::Fill, vec![4, 6]));
    }

    #[test]
    fn the_last_setting_of_a_node_wins() {
        let once = with_setting(&list(1, 4), Setting::Fibres, 0.5);
        let twice = with_setting(&concat_arg_vecs(vec![once, list(1, 8)]), Setting::Fibres, 2.0);
        let set = with_setting(&twice, Setting::Blend, 1.5);
        let mut paths = read_paths(&set, PathStyle::BODY);
        apply_settings(&mut paths, &read_settings(&set));
        let all = PathSettings { blend: Some(1.5), fibres: Some(2.0), fill: None };
        assert_eq!(paths.iter().map(|p| p.settings).collect::<Vec<_>>(), vec![all, all]);

        let only_four = with_setting(&concat_arg_vecs(vec![with_setting(&list(1, 4), Setting::Fill, 1.0), list(1, 8)]), Setting::Blend, 2.0);
        let mut paths = read_paths(&only_four, PathStyle::BODY);
        apply_settings(&mut paths, &read_settings(&only_four));
        assert_eq!(paths[0].settings, PathSettings { blend: Some(2.0), fibres: None, fill: Some(1.0) });
        assert_eq!(paths[1].settings, PathSettings { blend: Some(2.0), fibres: None, fill: None });
    }

    #[test]
    fn items_round_trip() {
        let item = SettingItem { setting: Setting::Fill, value: 6.5, creators: vec![3, -1, 70000] };
        assert_eq!(SettingItem::decode(&wrap_arg(&item.encode())), Some(item));
        assert_eq!(SettingItem::decode(&wrap_arg(&[SETTING_TAG, 7, 0])), None);
        assert_eq!(SettingItem::decode(split_args(&list(1, 1))[0]), None);
    }
}
