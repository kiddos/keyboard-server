use std::env;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

fn main() {
    #[cfg(target_os = "linux")]
    {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let pc_dir = out_dir.join("pkgconfig");
        let lib_sym_dir = out_dir.join("lib");
        fs::create_dir_all(&pc_dir).unwrap();
        fs::create_dir_all(&lib_sym_dir).unwrap();

        let lib_dir = if std::path::Path::new("/usr/lib/x86_64-linux-gnu").exists() {
            "/usr/lib/x86_64-linux-gnu"
        } else if std::path::Path::new("/usr/lib64").exists() {
            "/usr/lib64"
        } else {
            "/usr/lib"
        };

        let libs = [
            ("libXi.so", "libXi.so.6"),
            ("libXtst.so", "libXtst.so.6"),
            ("libdbus-1.so", "libdbus-1.so.3"),
            ("libX11.so", "libX11.so.6"),
        ];

        for (sym_name, target_name) in libs {
            let sym_path = lib_sym_dir.join(sym_name);
            let target_path = PathBuf::from(lib_dir).join(target_name);
            if target_path.exists() && !sym_path.exists() {
                let _ = symlink(&target_path, &sym_path);
            }
        }

        println!("cargo:rustc-link-search=native={}", lib_sym_dir.display());
        println!("cargo:rustc-link-search=native={}", lib_dir);

        let xi_pc = format!(
            "prefix=/usr\nexec_prefix=${{prefix}}\nlibdir={}\nincludedir=${{prefix}}/include\n\nName: Xi\nDescription: X Input Extension Library\nVersion: 1.8\nRequires: x11\nCflags: -I${{includedir}}\nLibs: -L${{libdir}} -lXi\n",
            lib_sym_dir.display()
        );
        fs::write(pc_dir.join("xi.pc"), xi_pc).unwrap();

        let xtst_pc = format!(
            "prefix=/usr\nexec_prefix=${{prefix}}\nlibdir={}\nincludedir=${{prefix}}/include\n\nName: Xtst\nDescription: X Test Extension Library\nVersion: 1.2\nRequires: x11\nCflags: -I${{includedir}}\nLibs: -L${{libdir}} -lXtst\n",
            lib_sym_dir.display()
        );
        fs::write(pc_dir.join("xtst.pc"), xtst_pc).unwrap();

        let dbus_pc = format!(
            "prefix=/usr\nexec_prefix=${{prefix}}\nlibdir={}\nincludedir=${{prefix}}/include\n\nName: dbus\nDescription: Free desktop message bus\nVersion: 1.12.20\nLibs: -L${{libdir}} -ldbus-1\nCflags: -I${{includedir}}/dbus-1.0 -I${{libdir}}/dbus-1.0/include\n",
            lib_sym_dir.display()
        );
        fs::write(pc_dir.join("dbus-1.pc"), dbus_pc).unwrap();

        let current_pkg_config = env::var("PKG_CONFIG_PATH").unwrap_or_default();
        let new_pkg_config = if current_pkg_config.is_empty() {
            pc_dir.to_string_lossy().to_string()
        } else {
            format!("{}:{}", pc_dir.display(), current_pkg_config)
        };
        env::set_var("PKG_CONFIG_PATH", new_pkg_config);
    }
}
