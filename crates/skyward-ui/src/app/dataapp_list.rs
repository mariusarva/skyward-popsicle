//! 数据分区里装着的应用：读、看、卸。

use crate::app::dataapp::DataApp;
use crate::app::App;

impl App {
    /// 某一页里第一条与查询对得上的条目：应用按名字或包名，模块按名字或目录名。
    pub fn first_match(&self, scope: crate::app::SearchScope, query: &str) -> Option<usize> {
        if query.is_empty() {
            return None;
        }
        match scope {
            crate::app::SearchScope::Apps => match &self.state.apps {
                Some(Ok(list)) => list
                    .iter()
                    .position(|a| a.label.to_lowercase().contains(query) || a.package.to_lowercase().contains(query)),
                _ => None,
            },
            crate::app::SearchScope::Modules => match &self.state.modules {
                Some(Ok(list)) => list
                    .iter()
                    .position(|m| m.name.to_lowercase().contains(query) || m.id.to_lowercase().contains(query)),
                _ => None,
            },
            crate::app::SearchScope::Features => None,
        }
    }

    /// The apps read for the page, or why they could not be.
    /// The apps read, and their icons already decoded: done on the reader's
    /// thread, since decoding a hundred icons would hold the pages still.
    pub fn set_apps(&mut self, list: Result<Vec<DataApp>, String>, icons: Vec<(u32, u32, Vec<u8>)>) {
        self.state.app_icons = icons;
        self.state.set_apps(list);
        self.refresh();
    }

    /// 一条的名字与图标补上了：`icon` 是解好的像素，空就是没有图标。
    pub fn data_app_enriched(&mut self, at: usize, label: String, icon: Option<(u32, u32, Vec<u8>)>) {
        let Some(Ok(list)) = &mut self.state.apps else { return };
        let Some(app) = list.get_mut(at) else { return };
        app.label = label;
        if let Some(pixels) = icon {
            if self.state.app_icons.len() <= at {
                self.state.app_icons.resize(at + 1, (0, 0, Vec::new()));
            }
            self.state.app_icons[at] = pixels;
        }
        // 这里不 refresh：回填是成批来的，每来一条就重画会把主循环占满，
        // 期间的点击要排队等（2026-11-18）。调用方在一批收完后 refresh 一次。
    }

    /// 系统分区的预装应用读回来了。
    pub fn set_sys_apps(&mut self, list: Vec<DataApp>) {
        self.state.sys_apps = Some(list);
        self.refresh();
    }

    /// The apps are being read: the page says so until they come.
    pub fn set_apps_pending(&mut self) {
        self.state.apps = None;
        self.refresh();
    }
}