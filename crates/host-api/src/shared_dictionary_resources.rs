//! 日文、五笔的只读资源可以与宿主私有的偏好、学习和缓存分开存放。

use crate::{absolute_state_root, DownloadedDictionaries, HostOptions};
use std::path::{Path, PathBuf};

impl HostOptions {
    pub(crate) fn dictionary_resource_root(&self) -> Option<PathBuf> {
        absolute_state_root(self.dictionary_resource_root.as_deref())
    }
}

impl<'a> DownloadedDictionaries<'a> {
    pub(crate) fn with_resource_root(mut self, root: Option<&'a Path>) -> Self {
        self.resource_root = root.unwrap_or(self.root);
        self
    }
}

#[cfg(test)]
mod tests;
