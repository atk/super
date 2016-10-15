use std::{u8, fs};
use std::path::{Path, PathBuf};
use std::convert::From;
use std::str::FromStr;
use std::io::Read;
use std::process::exit;
use std::collections::btree_set::Iter;
use std::slice::Iter as VecIter;
use std::collections::BTreeSet;
use std::cmp::{PartialOrd, Ordering};

use colored::Colorize;
use toml::{Parser, Value};

use static_analysis::manifest::Permission;

use {Error, Result, Criticity, print_error, print_warning};

const MAX_THREADS: i64 = u8::MAX as i64;

#[derive(Debug)]
pub struct Config {
    app_package: String,
    verbose: bool,
    quiet: bool,
    force: bool,
    bench: bool,
    open: bool,
    threads: u8,
    downloads_folder: PathBuf,
    dist_folder: PathBuf,
    results_folder: PathBuf,
    apktool_file: PathBuf,
    dex2jar_folder: PathBuf,
    jd_cmd_file: PathBuf,
    rules_json: PathBuf,
    templates_folder: PathBuf,
    template: String,
    unknown_permission: (Criticity, String),
    permissions: BTreeSet<PermissionConfig>,
    loaded_files: Vec<PathBuf>,
}

impl Config {
    #[cfg(target_family = "unix")]
    pub fn new<S: AsRef<str>>(app_package: S,
                              verbose: bool,
                              quiet: bool,
                              force: bool,
                              bench: bool,
                              open: bool)
                              -> Result<Config> {
        let mut config: Config = Default::default();
        config.app_package = String::from(app_package.as_ref());
        config.verbose = verbose;
        config.quiet = quiet;
        config.force = force;
        config.bench = bench;
        config.open = open;

        if Path::new("/etc/config.toml").exists() {
            try!(Config::load_from_file(&mut config, "/etc/config.toml", verbose));
            config.loaded_files.push(PathBuf::from("/etc/config.toml"));
        }
        if Path::new("./config.toml").exists() {
            try!(Config::load_from_file(&mut config, "./config.toml", verbose));
            config.loaded_files.push(PathBuf::from("./config.toml"));
        }

        Ok(config)
    }

    #[cfg(target_family = "windows")]
    pub fn new<S: AsRef<str>>(app_package: S,
                              verbose: bool,
                              quiet: bool,
                              force: bool,
                              bench: bool,
                              open: bool)
                              -> Result<Config> {
        let mut config: Config = Default::default();
        config.app_package = String::from(app_package.as_ref());
        config.verbose = verbose;
        config.quiet = quiet;
        config.force = force;
        config.bench = bench;
        config.open = open;

        if Path::new("config.toml").exists() {
            try!(Config::load_from_file(&mut config, "config.toml", verbose));
            config.loaded_files.push(PathBuf::from("config.toml"));
        }

        Ok(config)
    }

    pub fn check(&self) -> bool {
        self.downloads_folder.exists() && self.get_apk_file().exists() &&
        self.apktool_file.exists() && self.dex2jar_folder.exists() &&
        self.jd_cmd_file.exists() && self.get_template_path().exists() &&
        self.rules_json.exists()
    }

    pub fn get_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !self.downloads_folder.exists() {
            errors.push(format!("the downloads folder `{}` does not exist",
                                self.downloads_folder.display()));
        }
        if !self.get_apk_file().exists() {
            errors.push(format!("the APK file `{}` does not exist",
                                self.get_apk_file().display()));
        }
        if !self.apktool_file.exists() {
            errors.push(format!("the APKTool JAR file `{}` does not exist",
                                self.apktool_file.display()));
        }
        if !self.dex2jar_folder.exists() {
            errors.push(format!("the Dex2Jar folder `{}` does not exist",
                                self.dex2jar_folder.display()));
        }
        if !self.jd_cmd_file.exists() {
            errors.push(format!("the jd-cmd file `{}` does not exist",
                                self.jd_cmd_file.display()));
        }
        if !self.templates_folder.exists() {
            errors.push(format!("the templates folder `{}` does not exist",
                                self.templates_folder.display()));
        }
        if !self.get_template_path().exists() {
            errors.push(format!("the template `{}` does not exist in `{}`",
                                self.template,
                                self.templates_folder.display()));
        }
        if !self.rules_json.exists() {
            errors.push(format!("the `{}` rule file does not exist",
                                self.rules_json.display()));
        }
        errors
    }

    pub fn get_loaded_config_files(&self) -> VecIter<PathBuf> {
        self.loaded_files.iter()
    }

    pub fn get_app_package(&self) -> &str {
        &self.app_package
    }

    pub fn set_app_package<S: AsRef<str>>(&mut self, app_package: S) {
        self.app_package = app_package.as_ref().to_owned();
    }

    pub fn get_apk_file(&self) -> PathBuf {
        self.downloads_folder.join(format!("{}.apk", self.app_package))
    }

    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    pub fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    pub fn is_force(&self) -> bool {
        self.force
    }

    pub fn set_force(&mut self, force: bool) {
        self.force = force;
    }

    pub fn is_bench(&self) -> bool {
        self.bench
    }

    pub fn set_bench(&mut self, bench: bool) {
        self.bench = bench;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_open(&mut self, open: bool) {
        self.open = open;
    }

    pub fn get_threads(&self) -> u8 {
        self.threads
    }

    pub fn set_threads(&mut self, threads: u8) {
        self.threads = threads;
    }

    pub fn get_downloads_folder(&self) -> &Path {
        &self.downloads_folder
    }

    pub fn get_dist_folder(&self) -> &Path {
        &self.dist_folder
    }

    pub fn get_results_folder(&self) -> &Path {
        &self.results_folder
    }

    pub fn get_apktool_file(&self) -> &Path {
        &self.apktool_file
    }

    pub fn get_dex2jar_folder(&self) -> &Path {
        &self.dex2jar_folder
    }

    pub fn get_jd_cmd_file(&self) -> &Path {
        &self.jd_cmd_file
    }

    pub fn get_template_path(&self) -> PathBuf {
        self.templates_folder.join(&self.template)
    }

    pub fn get_templates_folder(&self) -> &Path {
        &self.templates_folder
    }

    pub fn get_template_name(&self) -> &str {
        &self.template
    }

    pub fn get_rules_json(&self) -> &Path {
        &self.rules_json
    }

    pub fn get_unknown_permission_criticity(&self) -> Criticity {
        self.unknown_permission.0
    }

    pub fn get_unknown_permission_description(&self) -> &str {
        self.unknown_permission.1.as_str()
    }

    pub fn get_permissions(&self) -> Iter<PermissionConfig> {
        self.permissions.iter()
    }

    fn load_from_file<P: AsRef<Path>>(config: &mut Config, path: P, verbose: bool) -> Result<()> {
        let mut f = try!(fs::File::open(path));
        let mut toml = String::new();
        let _ = try!(f.read_to_string(&mut toml));

        let mut parser = Parser::new(toml.as_str());
        let toml = match parser.parse() {
            Some(t) => t,
            None => {
                print_error(format!("There was an error parsing the config.toml file: {:?}",
                                    parser.errors),
                            verbose);
                exit(Error::ParseError.into());
            }
        };

        for (key, value) in toml {
            match key.as_str() {
                "threads" => {
                    match value {
                        Value::Integer(1...MAX_THREADS) => {
                            config.threads = value.as_integer().unwrap() as u8
                        }
                        _ => {
                            print_warning(format!("The 'threads' option in config.toml must \
                                                   be an integer between 1 and {}.\nUsing \
                                                   default.",
                                                  MAX_THREADS),
                                          verbose)
                        }
                    }
                }
                "downloads_folder" => {
                    match value {
                        Value::String(s) => config.downloads_folder = PathBuf::from(s),
                        _ => {
                            print_warning("The 'downloads_folder' option in config.toml must \
                                           be an string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "dist_folder" => {
                    match value {
                        Value::String(s) => config.dist_folder = PathBuf::from(s),
                        _ => {
                            print_warning("The 'dist_folder' option in config.toml must be an \
                                           string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "results_folder" => {
                    match value {
                        Value::String(s) => config.results_folder = PathBuf::from(s),
                        _ => {
                            print_warning("The 'results_folder' option in config.toml must be \
                                           an string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "apktool_file" => {
                    match value {
                        Value::String(s) => {
                            let extension = Path::new(&s).extension();
                            if extension.is_some() && extension.unwrap() == "jar" {
                                config.apktool_file = PathBuf::from(s.clone());
                            } else {
                                print_warning("The APKTool file must be a JAR file.\nUsing \
                                               default.",
                                              verbose)
                            }
                        }
                        _ => {
                            print_warning("The 'apktool_file' option in config.toml must be \
                                           an string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "dex2jar_folder" => {
                    match value {
                        Value::String(s) => config.dex2jar_folder = PathBuf::from(s),
                        _ => {
                            print_warning("The 'dex2jar_folder' option in config.toml should \
                                           be an string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "jd_cmd_file" => {
                    match value {
                        Value::String(s) => {
                            let extension = Path::new(&s).extension();
                            if extension.is_some() && extension.unwrap() == "jar" {
                                config.jd_cmd_file = PathBuf::from(s.clone());
                            } else {
                                print_warning("The JD-CMD file must be a JAR file.\nUsing \
                                               default.",
                                              verbose)
                            }
                        }
                        _ => {
                            print_warning("The 'jd_cmd_file' option in config.toml must be an \
                                           string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "templates_folder" => {
                    match value {
                        Value::String(s) => config.templates_folder = PathBuf::from(s),
                        _ => {
                            print_warning("The 'templates_folder' option in config.toml \
                                           should be an string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "template" => {
                    match value {
                        Value::String(s) => config.template = s,
                        _ => {
                            print_warning("The 'template' option in config.toml \
                                           should be an string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "rules_json" => {
                    match value {
                        Value::String(s) => {
                            let extension = Path::new(&s).extension();
                            if extension.is_some() && extension.unwrap() == "json" {
                                config.rules_json = PathBuf::from(s.clone());
                            } else {
                                print_warning("The rules.json file must be a JSON \
                                               file.\nUsing default.",
                                              verbose)
                            }
                        }
                        _ => {
                            print_warning("The 'rules_json' option in config.toml must be an \
                                           string.\nUsing default.",
                                          verbose)
                        }
                    }
                }
                "permissions" => {
                    match value {
                        Value::Array(p) => {
                            let format_warning =
                                format!("The permission configuration format must be the \
                                         following:\n{}\nUsing default.",
                                        "[[permissions]]\nname=\"unknown|permission.name\"\n\
                                        criticity = \"warning|low|medium|high|critical\"\n\
                                        label = \"Permission label\"\n\
                                        description = \"Long description to explain the \
                                        vulnerability\""
                                            .italic());

                            for cfg in p {
                                let cfg = match cfg.as_table() {
                                    Some(t) => t,
                                    None => {
                                        print_warning(format_warning, verbose);
                                        break;
                                    }
                                };

                                let name = match cfg.get("name") {
                                    Some(&Value::String(ref n)) => n,
                                    _ => {
                                        print_warning(format_warning, verbose);
                                        break;
                                    }
                                };

                                let criticity = match cfg.get("criticity") {
                                    Some(&Value::String(ref c)) => {
                                        match Criticity::from_str(c) {
                                            Ok(c) => c,
                                            Err(_) => {
                                                print_warning(format!("Criticity must be \
                                                                       one of {}, {}, {}, \
                                                                       {} or {}.\nUsing \
                                                                       default.",
                                                                      "warning".italic(),
                                                                      "low".italic(),
                                                                      "medium".italic(),
                                                                      "high".italic(),
                                                                      "critical".italic()),
                                                              verbose);
                                                break;
                                            }
                                        }
                                    }
                                    _ => {
                                        print_warning(format_warning, verbose);
                                        break;
                                    }
                                };

                                let description = match cfg.get("description") {
                                    Some(&Value::String(ref d)) => d,
                                    _ => {
                                        print_warning(format_warning, verbose);
                                        break;
                                    }
                                };

                                if name == "unknown" {
                                    if cfg.len() != 3 {
                                        print_warning(format!("The format for the unknown \
                                        permissions is the following:\n{}\nUsing default.",
                                        "[[permissions]]\nname = \"unknown\"\n\
                                        criticity = \"warning|low|medium|high|criticity\"\n\
                                        description = \"Long description to explain the \
                                        vulnerability\"".italic()),
                                                      verbose);
                                        break;
                                    }

                                    config.unknown_permission = (criticity, description.clone());
                                } else {
                                    if cfg.len() != 4 {
                                        print_warning(format_warning, verbose);
                                        break;
                                    }

                                    let permission = match Permission::from_str(name.as_str()) {
                                        Ok(p) => p,
                                        Err(_) => {
                                            print_warning(format!("Unknown permission: {}\nTo \
                                                                   set the default \
                                                                   vulnerability level for an \
                                                                   unknown permission, please, \
                                                                   use the {} permission name, \
                                                                   under the {} section.",
                                                                  name.italic(),
                                                                  "unknown".italic(),
                                                                  "[[permissions]]".italic()),
                                                          verbose);
                                            break;
                                        }
                                    };

                                    let label = match cfg.get("label") {
                                        Some(&Value::String(ref l)) => l,
                                        _ => {
                                            print_warning(format_warning, verbose);
                                            break;
                                        }
                                    };
                                    config.permissions
                                        .insert(PermissionConfig::new(permission,
                                                                      criticity,
                                                                      label,
                                                                      &String::from(
                                                                          description.as_ref())));
                                }
                            }
                        }
                        _ => {
                            print_warning("You must specify the permissions you want to \
                                           select as vulnerable.",
                                          verbose)
                        }
                    }
                }
                _ => print_warning(format!("Unknown configuration option {}.", key), verbose),
            }
        }
        Ok(())
    }

    fn local_default() -> Config {
        Config {
            app_package: String::new(),
            verbose: false,
            quiet: false,
            force: false,
            bench: false,
            open: false,
            threads: 2,
            downloads_folder: PathBuf::from("downloads"),
            dist_folder: PathBuf::from("dist"),
            results_folder: PathBuf::from("results"),
            apktool_file: Path::new("vendor").join("apktool_2.2.0.jar"),
            dex2jar_folder: Path::new("vendor").join("dex2jar-2.0"),
            jd_cmd_file: Path::new("vendor").join("jd-cmd.jar"),
            templates_folder: PathBuf::from("templates"),
            template: String::from("super"),
            rules_json: PathBuf::from("rules.json"),
            unknown_permission: (Criticity::Low,
                                 String::from("Even if the application can create its own \
                                               permissions, it's discouraged, since it can \
                                               lead to missunderstanding between developers.")),
            permissions: BTreeSet::new(),
            loaded_files: Vec::new(),
        }
    }
}

impl Default for Config {
    #[cfg(target_family = "unix")]
    fn default() -> Config {
        let mut config = Config::local_default();
        let etc_rules = PathBuf::from("/etc/super/rules.json");
        if etc_rules.exists() {
            config.rules_json = etc_rules;
        }
        let share_path = Path::new(if cfg!(target_os = "macos") {
            "/usr/local/super"
        } else {
            "/usr/share/super"
        });
        if share_path.exists() {
            config.apktool_file = share_path.join("vendor/apktool_2.2.0.jar");
            config.dex2jar_folder = share_path.join("vendor/dex2jar-2.0");
            config.jd_cmd_file = share_path.join("vendor/jd-cmd.jar");
            config.templates_folder = share_path.join("templates");
        }
        config
    }

    #[cfg(target_family = "windows")]
    fn default() -> Config {
        Config::local_default()
    }
}

#[derive(Debug, Ord, Eq)]
pub struct PermissionConfig {
    permission: Permission,
    criticity: Criticity,
    label: String,
    description: String,
}

impl PartialEq for PermissionConfig {
    fn eq(&self, other: &PermissionConfig) -> bool {
        self.permission == other.permission
    }
}

impl PartialOrd for PermissionConfig {
    fn partial_cmp(&self, other: &PermissionConfig) -> Option<Ordering> {
        if self.permission < other.permission {
            Some(Ordering::Less)
        } else if self.permission > other.permission {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}

impl PermissionConfig {
    fn new<S: AsRef<str>>(permission: Permission,
                          criticity: Criticity,
                          label: S,
                          description: S)
                          -> PermissionConfig {
        PermissionConfig {
            permission: permission,
            criticity: criticity,
            label: String::from(label.as_ref()),
            description: String::from(description.as_ref()),
        }
    }

    pub fn get_permission(&self) -> Permission {
        self.permission
    }

    pub fn get_criticity(&self) -> Criticity {
        self.criticity
    }

    pub fn get_label(&self) -> &str {
        self.label.as_str()
    }

    pub fn get_description(&self) -> &str {
        self.description.as_str()
    }
}

#[cfg(test)]
mod tests {
    use Criticity;
    use static_analysis::manifest::Permission;
    use super::Config;
    use std::fs;
    use std::path::Path;

    #[test]
    fn it_config() {
        let mut config: Config = Default::default();

        assert_eq!(config.get_app_package(), "");
        assert!(!config.is_verbose());
        assert!(!config.is_quiet());
        assert!(!config.is_force());
        assert!(!config.is_bench());
        assert!(!config.is_open());
        assert_eq!(config.get_threads(), 2);
        assert_eq!(config.get_downloads_folder(), Path::new("downloads"));
        assert_eq!(config.get_dist_folder(), Path::new("dist"));
        assert_eq!(config.get_results_folder(), Path::new("results"));
        assert_eq!(config.get_template_name(), "super");
        let share_path = Path::new(if cfg!(target_os = "macos") {
            "/usr/local/super"
        } else if cfg!(target_family = "windows") {
            ""
        } else {
            "/usr/share/super"
        });
        let share_path = if share_path.exists() {
            share_path
        } else {
            Path::new("")
        };
        assert_eq!(config.get_apktool_file(),
                   share_path.join("vendor").join("apktool_2.2.0.jar"));
        assert_eq!(config.get_dex2jar_folder(),
                   share_path.join("vendor").join("dex2jar-2.0"));
        assert_eq!(config.get_jd_cmd_file(),
                   share_path.join("vendor").join("jd-cmd.jar"));
        assert_eq!(config.get_templates_folder(), share_path.join("templates"));
        assert_eq!(config.get_template_path(),
                   share_path.join("templates").join("super"));
        if cfg!(target_family = "unix") && Path::new("/etc/super/rules.json").exists() {
            assert_eq!(config.get_rules_json(), Path::new("/etc/super/rules.json"));
        } else {
            assert_eq!(config.get_rules_json(), Path::new("rules.json"));
        }
        assert_eq!(config.get_unknown_permission_criticity(), Criticity::Low);
        assert_eq!(config.get_unknown_permission_description(),
                   "Even if the application can create its own permissions, it's discouraged, \
                    since it can lead to missunderstanding between developers.");
        assert_eq!(config.get_permissions().next(), None);

        if !config.get_downloads_folder().exists() {
            fs::create_dir(config.get_downloads_folder()).unwrap();
        }
        if !config.get_dist_folder().exists() {
            fs::create_dir(config.get_dist_folder()).unwrap();
        }
        if !config.get_results_folder().exists() {
            fs::create_dir(config.get_results_folder()).unwrap();
        }

        config.set_app_package("test_app");
        config.set_verbose(true);
        config.set_quiet(true);
        config.set_force(true);
        config.set_bench(true);
        config.set_open(true);

        assert_eq!(config.get_app_package(), "test_app");
        assert!(config.is_verbose());
        assert!(config.is_quiet());
        assert!(config.is_force());
        assert!(config.is_bench());
        assert!(config.is_open());

        if config.get_apk_file().exists() {
            fs::remove_file(config.get_apk_file()).unwrap();
        }
        assert!(!config.check());

        let _ = fs::File::create(config.get_apk_file()).unwrap();
        assert!(config.check());

        let config = Config::new("test_app", false, false, false, false, false).unwrap();
        let mut error_string = String::from("Configuration errors were found:\n");
        for error in config.get_errors() {
            error_string.push_str(&error);
            error_string.push('\n');
        }
        error_string.push_str("The configuration was loaded, in order, from the following \
                               files:\n\t- Default built-in configuration\n");
        for file in config.get_loaded_config_files() {
            error_string.push_str(&format!("\t- {}\n", file.display()));
        }
        println!("{}", error_string);
        assert!(config.check());

        fs::remove_file(config.get_apk_file()).unwrap();
    }

    #[test]
    fn it_config_sample() {
        let mut config = Config::default();
        Config::load_from_file(&mut config, "config.toml.sample", false).unwrap();
        config.set_app_package("test_app");

        assert_eq!(config.get_threads(), 2);
        assert_eq!(config.get_downloads_folder(), Path::new("downloads"));
        assert_eq!(config.get_dist_folder(), Path::new("dist"));
        assert_eq!(config.get_results_folder(), Path::new("results"));
        assert_eq!(config.get_apktool_file(),
                   Path::new("/usr/share/super/vendor/apktool_2.2.0.jar"));
        assert_eq!(config.get_dex2jar_folder(),
                   Path::new("/usr/share/super/vendor/dex2jar-2.0"));
        assert_eq!(config.get_jd_cmd_file(),
                   Path::new("/usr/share/super/vendor/jd-cmd.jar"));
        assert_eq!(config.get_templates_folder(),
                   Path::new("/usr/share/super/templates"));
        assert_eq!(config.get_template_path(),
                   Path::new("/usr/share/super/templates/super"));
        assert_eq!(config.get_template_name(), "super");
        assert_eq!(config.get_rules_json(), Path::new("/etc/super/rules.json"));
        assert_eq!(config.get_unknown_permission_criticity(), Criticity::Low);
        assert_eq!(config.get_unknown_permission_description(),
                   "Even if the application can create its own permissions, it's discouraged, \
                    since it can lead to missunderstanding between developers.");

        let permission = config.get_permissions().next().unwrap();
        assert_eq!(permission.get_permission(),
                   Permission::AndroidPermissionInternet);
        assert_eq!(permission.get_criticity(), Criticity::Warning);
        assert_eq!(permission.get_label(), "Internet permission");
        assert_eq!(permission.get_description(),
                   "Allows the app to create network sockets and use custom network protocols. \
                    The browser and other applications provide means to send data to the \
                    internet, so this permission is not required to send data to the internet. \
                    Check if the permission is actually needed.");
    }
}
