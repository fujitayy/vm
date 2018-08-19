//! vm
//!
//! ## ユースケース
//!
//! * vmの一覧を表示
//!     * `vm list`
//! * vmを追加。パスは絶対パスに変換して登録される。同名の登録があった場合はそれを表示してエラー終了
//!     * `vm add NAME PATH`
//! * vmの設定ファイルの絶対パスを表示。
//!     * `vm config-file-path`
//! * vmの設定を削除。(y/N)の確認プロンプトを出す。
//!     * `vm remove NAME`
//!     * `vm remove NAME --force` (--forceで確認無しで削除)
//! * vmの設定をバックアップ。現在日時をファイル名の末尾に付けてバックアップする。バックアップ先は設定ファイルがあるのと同じディレクトリ。
//!     * `vm backup-config-file`
//! * 特定のパス以下に存在しているVagrantfileを探してそのパスを表示する
//!     * `vm find-vagrantfile`
//! * 任意オプションでvagrantコマンドを実行する。(pluginのコマンド等の非標準コマンドの実行に使用)
//!     * `vm NAME -c vbguest`
//!     * `vm NAME -c 'vbguest --do rebuild'`
//!
//! ## 将来的に必要性を感じたら作る
//! * ある名前のvmに対してvagrantコマンドのサブコマンドを実行
//!     * `vm NAME up`
//!     * `vm NAME ssh`
//! * ある名前のvmに対してvagrantコマンドのサブコマンドを引数付きで実行
//!     * `vm NAME ssh --vagrant-opt '-- -L 8080:localhost:8080'`
//!     * `vm NAME ssh -o '-- -L 8080:localhost:8080'`
//! * いくつかのvmコマンドレベルで対応しているvagrantコマンドのサブコマンドの引数を指定して実行
//!     * `vm NAME ssh -L 8080:localhost:8080`

extern crate clap;
#[macro_use]
extern crate failure;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use failure::Error;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

/// configファイル中のあるVMに関する情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    name: String,
    path: PathBuf,
}

impl Info {
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

/// configファイルの情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    vagrant_path: PathBuf,
    vm_list: BTreeMap<String, Info>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Config, Error> {
        if path.as_ref().exists() {
            let file = File::open(path)?;
            let mut buf = Vec::new();
            let _ = BufReader::new(file).read_to_end(&mut buf)?;
            Ok(toml::from_slice(&buf)?)
        } else {
            let vagrant_path = if cfg!(windows) {
                "vagrant.exe"
            } else {
                "vagrant"
            };
            Ok(Config {
                vagrant_path: PathBuf::from(vagrant_path),
                vm_list: BTreeMap::new(),
            })
        }
    }

    pub fn vagrant_path(&self) -> &Path {
        self.vagrant_path.as_path()
    }

    pub fn vm_list(&self) -> &BTreeMap<String, Info> {
        &self.vm_list
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        BufWriter::new(File::create(path)?).write_all(toml::to_string_pretty(self)?.as_bytes())?;
        Ok(())
    }
}

pub trait RunVagrant {
    fn subcommand<S: AsRef<OsStr>, T: AsRef<OsStr>>(
        &self,
        command: S,
        options: &[T],
    ) -> Result<ExitStatus, Error>;

    fn raw<S: AsRef<OsStr>>(&self, options: &[S]) -> Result<ExitStatus, Error>;
}

pub struct Vagrant {
    path: PathBuf,
}

impl Vagrant {
    pub fn new<P: AsRef<Path>>(path: P) -> Vagrant {
        Vagrant { path: path.as_ref().to_path_buf() }
    }
}

impl RunVagrant for Vagrant {
    fn subcommand<S: AsRef<OsStr>, T: AsRef<OsStr>>(
        &self,
        command: S,
        options: &[T],
    ) -> Result<ExitStatus, Error> {
        Ok(Command::new(&self.path)
            .arg(command.as_ref())
            .args(options)
            .status()?)
    }

    /// Pass vagrant command options.
    fn raw<S: AsRef<OsStr>>(&self, options: &[S]) -> Result<ExitStatus, Error> {
        Ok(Command::new(&self.path).args(options).status()?)
    }
}

#[derive(Debug, Clone, Fail)]
#[fail(display = "cannot find {} from vm list in config file", name)]
pub struct VmInfoFindError {
    name: String,
}

pub struct Vm<V: RunVagrant> {
    config_file_path: PathBuf,
    config: Config,
    vagrant: V,
}

impl<V: RunVagrant> Vm<V> {
    pub fn new<P: AsRef<Path>>(path: P, config: Config, vagrant: V) -> Result<Vm<V>, Error> {
        Ok(Vm {
            config_file_path: path.as_ref().to_path_buf(),
            config,
            vagrant,
        })
    }

    pub fn config_file_path(&self) -> &Path {
        self.config_file_path.as_path()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn cd<S: AsRef<str>>(&self, name: S) -> Result<(), Error> {
        let info = self
            .config
            .vm_list
            .get(name.as_ref())
            .ok_or_else(|| VmInfoFindError {
                name: name.as_ref().to_string(),
            })?;
        std::env::set_current_dir(&info.path)?;
        Ok(())
    }

    pub fn list(&self) -> Vec<&Info> {
        self.config.vm_list.values().collect()
    }

    /// Add a new entry and return old value that assigned to the `name` if exists.
    pub fn add<S: AsRef<str>, P: AsRef<Path>>(&mut self, name: S, path: P) -> Option<Info> {
        let info = Info {
            name: name.as_ref().to_string(),
            path: path.as_ref().to_path_buf(),
        };
        self.config.vm_list.insert(info.name.clone(), info)
    }

    pub fn remove<S: AsRef<str>>(&mut self, name: S) -> Option<Info> {
        self.config.vm_list.remove(name.as_ref())
    }

    pub fn backup_config_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let _ = std::fs::copy(self.config_file_path(), path)?;
        Ok(())
    }

    pub fn vagrant<S: AsRef<OsStr>, T: AsRef<OsStr>>(
        &self,
        command: S,
        options: &[T],
    ) -> Result<ExitStatus, Error> {
        Ok(self.vagrant.subcommand(command, options)?)
    }

    /// Pass vagrant command options.
    pub fn vagrant_raw<S: AsRef<OsStr>>(&self, options: &[S]) -> Result<ExitStatus, Error> {
        Ok(self.vagrant.raw(options)?)
    }

    pub fn get_info<S: AsRef<str>>(&self, name: S) -> Option<&Info> {
        self.config.vm_list.get(name.as_ref())
    }
}

/// Does not traverse symlink.
pub fn find_vagrantfiles<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    if !path.as_ref().is_dir() {
        return Ok(());
    }

    for result in std::fs::read_dir(path)? {
        let item = result?;
        if item.file_name().to_string_lossy() == "Vagrantfile" {
            println!("{}", item.path().to_string_lossy());
        } else if item.path().is_dir()
            && item
                .path()
                .metadata()
                .map(|m| !m.file_type().is_symlink())
                .unwrap_or(false)
        {
            find_vagrantfiles(item.path())?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    fn vagrant_file_path() -> &'static str {
        "vagrant"
    }

    #[cfg(windows)]
    fn vagrant_file_path() -> &'static str {
        "vagrant.exe"
    }

    fn test_config_content_normal() -> String {
        format!(r#"
vagrant_file_path = "{}"

[vm_list]
vm1 = "/home/user/vm/vm1"
vm2 = "/home/user/vm/vm2"
        "#, vagrant_file_path())
    }

    fn create_test_dir() {
        std::fs::create_dir_all("test").unwrap();
    }

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
