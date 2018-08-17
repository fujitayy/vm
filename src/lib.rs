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
//! * ある名前のvmに対してvagrantコマンドのサブコマンドを実行
//!     * `vm NAME up`
//!     * `vm NAME ssh`
//! * ある名前のvmに対してvagrantコマンドのサブコマンドを引数付きで実行
//!     * `vm NAME ssh -- -- -L 8080:localhost:8080` (最初の -- 以降のオプションがそのまま渡される)
//! * いくつかのvmコマンドレベルで対応しているvagrantコマンドのサブコマンドの引数を指定して実行
//!     * `vm NAME ssh -L 8080:localhost:8080`
//! * 特定のパス以下に存在しているVagrantfileを探してそのパスを表示する
//!     * `vm find-vagrantfile`

extern crate clap;
exter crate failure;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

use failure::Error;
use std::path::{PathBuf, Path};

/// configファイルの情報
pub struct Config {
    vm_list: Vec<Info>,
}

/// configファイル中のあるVMに関する情報
pub struct Info {
    name: String,
    path: PathBuf,
}

pub struct Vm {
    config_file: PathBuf,
    config: Config,
}

impl Vm {
    pub fn list(&self) -> Result<Info, Error> {
        unimplemented!()
    }

    pub fn add<S: AsRef<str>, P: AsRef<Path>>(&self, name: S, path: P) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn config_file_path(&self) -> Result<PathBuf, Error> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
