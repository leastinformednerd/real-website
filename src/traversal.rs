use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs;
use std::io::Result as IoR;
use std::path::PathBuf;

#[derive(Debug)]
pub struct CategoryTree<'a> {
    pub name: String,
    pub child_dirs: Vec<CategoryTree<'a>>,
    pub child_docs: Vec<crate::parser::File<'a>>,
}

pub fn traverse(from: PathBuf) -> IoR<CategoryTree<'static>> {
    let mut child_dirs = Vec::new();
    let mut child_docs = Vec::new();

    let dir = fs::read_dir(&from)?;
    for entry in dir {
        handle_entry(entry?, &mut child_dirs, &mut child_docs)?;
    }

    Ok(CategoryTree {
        name: match from.file_stem() {
            Some(ostr) => ostr.to_string_lossy().into_owned(),
            None => "Unknown".into(),
        },
        child_dirs,
        child_docs,
    })
}

pub fn handle_entry(
    entry: fs::DirEntry,
    dirs: &mut Vec<CategoryTree<'static>>,
    docs: &mut Vec<crate::parser::File<'static>>,
) -> IoR<()> {
    let path = entry.path();
    if let Some(name) = path.file_name()
        && name.as_encoded_bytes().get(0) == Some(&b'.')
    {
        return Ok(());
    }
    match entry.file_type() {
        Ok(ft) => {
            if ft.is_dir() {
                dirs.push(traverse(entry.path())?);
            } else if let Some(ext) = path.extension()
                && let [b'm', b'd'] = ext.as_encoded_bytes()
            {
                docs.push(
                    match crate::parser::parse_file(Box::leak(
                        std::fs::read_to_string(entry.path())?.into_boxed_str(),
                    )) {
                        Ok(file) => file,
                        Err(e) => {
                            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e));
                        }
                    },
                )
            }
        }
        _ => todo!(),
    }

    Ok(())
}
