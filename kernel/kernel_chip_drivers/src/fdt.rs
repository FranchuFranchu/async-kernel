/// Code to construct and traverse a flattened device tree
// See https://github.com/devicetree-org/devicetree-specification/releases/tag/v0.3
use alloc::{
    borrow::ToOwned,
    collections::{BTreeMap, VecDeque},
    format,
    string::{String, ToString},
};
use core::{any::Any, mem::MaybeUninit};

use kernel_lock::shared::RwLock;
use kernel_printer::println;

use log::warn;
use cstr_core::CStr;
use num_enum::{FromPrimitive, IntoPrimitive};


static mut DEVICE_TREE_BASE: *const FdtHeader = core::ptr::null();
static mut DEVICE_TREE_ROOT: MaybeUninit<RwLock<Node>> = MaybeUninit::uninit();

#[derive(Debug)]
pub struct Node {
    pub name: &'static str,
    pub unit_address: Option<usize>,
    pub children: BTreeMap<&'static str, BTreeMap<Option<usize>, Node>>,
    pub properties: BTreeMap<&'static str, PropertyValue<'static>>,

    /// This holds an arbitrary datatype which is the representation of this type in the kernel
    /// This can (and is) used to own objects with functions that are called on interrupts
    pub kernel_struct: RwLock<Option<alloc::boxed::Box<dyn Any + Send>>>,
}
impl Node {
    pub fn new(token_name: &'static str) -> Self {
        let mut name_iter = token_name.splitn(2, '@');
        Self {
            name: name_iter.next().unwrap(),
            unit_address: name_iter
                .next()
                .map(|s| usize::from_str_radix(s, 16).unwrap_or(0)),
            children: BTreeMap::new(),
            properties: BTreeMap::new(),
            kernel_struct: RwLock::new(None),
        }
    }
    pub fn get<'this>(&'this self, path: &'this str) -> Option<&'this Node> {
        let mut path_iter = path.splitn(2, '/');
        let first_component = path_iter.next().unwrap_or(path);

        let child;

        // Get the device at a name + unit address, or just the name
        if first_component.contains('@') {
            // Full unit address
            let (name, address) = first_component.split_once('@').unwrap();
            if address.is_empty() {
                // A component ending with an @ allows any unit address (choose the first one then)
                child = self.children.get(name)?.values().next()?
            } else {
                child = self
                    .children
                    .get(name)?
                    .get(&Some(usize::from_str_radix(address, 16).unwrap_or(0)))?;
            }
        } else {
            child = self.children.get(first_component)?.get(&None)?;
        }

        // Recursion is done here
        if let Some(rest) = path_iter.next() {
            child.get(rest)
        } else {
            Some(child)
        }
    }
    pub fn children_names(&self) -> VecDeque<&'static str> {
        self.children.values().flatten().map(|s| s.1.name).collect()
    }
    pub fn children_names_address(&self) -> VecDeque<String> {
        self.children
            .values()
            .flatten()
            .map(|s| {
                if let Some(addr) = s.1.unit_address {
                    format!("{}@{:x}", s.1.name, addr)
                } else {
                    s.1.name.to_owned()
                }
            })
            .collect()
    }
    pub fn children(&self) -> VecDeque<&Node> {
        self.children.values().flatten().map(|s| s.1).collect()
    }
    pub fn children_mut(&mut self) -> VecDeque<&mut Node> {
        self.children.values_mut().flatten().map(|s| s.1).collect()
    }
    fn insert_child(&mut self, other: Self) {
        // Check if there's already a map for this name
        match self.children.get_mut(other.name) {
            Some(d) => {
                d.insert(other.unit_address, other);
            }
            None => {
                let mut map = BTreeMap::new();
                let name = other.name;
                map.insert(other.unit_address, other);
                self.children.insert(name, map);
            }
        }
    }
    pub fn pretty(&self, indent: usize) {
        match self.unit_address {
            Some(e) => println!("{}{}@{:x} {{", "    ".repeat(indent), self.name, e),
            None => println!("{}{} {{", "    ".repeat(indent), self.name),
        };
        // Add properties
        for (k, v) in self.properties.iter() {
            println!("{}{} = {}", "    ".repeat(indent + 1), k, v.as_str())
        }
        for i in self.children().iter() {
            i.pretty(indent + 1);
        }
        println!("{}}}", "    ".repeat(indent));
    }
    pub fn walk<F: FnMut(&'static Node)>(&'static self, closure: &mut F) {
        closure(self);
        for i in self.children() {
            i.walk(closure);
        }
    }

    pub fn walk_nonstatic<F: FnMut(&Node)>(&self, closure: &mut F) {
        closure(self);
        for i in self.children() {
            i.walk_nonstatic(closure);
        }
    }

    /// The lifetimes for this function aren't <'static> because that would be an aliasing rule violation
    /// (closure mutably borrows Node forever so no one else can mut borrow it again )
    pub fn walk_mut<F: FnMut(&mut Node)>(&mut self, closure: &mut F) {
        closure(self);
        for i in self.children_mut() {
            i.walk_mut(closure);
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        warn!("Node dropped (this doesn't happen with the current implementation)");
    }
}

#[derive(Debug)]
// this makes more sense here
#[allow(non_camel_case_types)]

pub enum PropertyValue<'data> {
    Empty,
    PHandleRaw(u32),
    PHandle(&'static Node),
    u32(u32),
    u64(u64),
    PropSpecific(&'data [u8]),
    String(&'data str),
    StringList(VecDeque<&'data str>),
}

impl<'data> PropertyValue<'data> {
    fn guess(value: &'data [u8], name: Option<&str>) -> PropertyValue<'data> {
        if value.is_empty() {
            return PropertyValue::Empty;
        }
        // Check if it has any bytes < 0x20
        // (or if there are two zeroes next to each other)
        let mut is_string = true;
        let mut is_string_list = false;

        let mut prev_is_zero = true;
        for &i in &value[..value.len() - 1] {
            if i == 0 && !prev_is_zero {
                is_string_list = true;
                prev_is_zero = true;
            } else if i < 0x20 {
                is_string = false;
                is_string_list = false;
                break;
            } else {
                prev_is_zero = false;
            }
        }
        if is_string_list {
            let mut list = VecDeque::new();
            let mut last_index = 0;
            for (index, &i) in value.iter().enumerate() {
                if i == 0 {
                    list.push_back(core::str::from_utf8(&value[last_index..index]).unwrap());
                    last_index = index + 1;
                }
            }
            return PropertyValue::StringList(list);
        } else if is_string {
            return PropertyValue::String(core::str::from_utf8(&value[..value.len() - 1]).unwrap());
        }
        // TODO Safe way for this?
        if value.len() == 8 {
            return PropertyValue::u64(unsafe { *(value.as_ptr() as *const u64) }.swap_bytes());
        };
        if value.len() == 4 {
            if name.unwrap_or("") == "interrupt-parent" {
                return PropertyValue::PHandleRaw(
                    unsafe { *(value.as_ptr() as *const u32) }.swap_bytes(),
                );
            }
            return PropertyValue::u32(unsafe { *(value.as_ptr() as *const u32) }.swap_bytes());
        };
        PropertyValue::PropSpecific(value)
    }
    fn as_str(&self) -> String {
        match self {
            Self::Empty => "true".to_string(),
            Self::PropSpecific(val) => format!("{:?}", val),
            Self::StringList(val) => format!("{:?}", val),
            Self::String(val) => val.to_string(),
            Self::u32(val) | Self::PHandleRaw(val) => format!("{:x}", val),
            Self::PHandle(val) => format!("<{}>", val.name),
            Self::u64(val) => format!("{:x}", val),
        }
    }
}

#[repr(C)]
pub struct FdtHeader {
    magic: u32,
    total_size: u32,
    offset_dt_struct: u32,
    offset_dt_strings: u32,
    offset_memory_reservemap: u32,
    version: u32,
    last_compatible_version: u32,
    boot_cpuid: u32,
    size_dt_strings: u32,
    size_dt_struct: u32,
}

#[derive(IntoPrimitive, FromPrimitive, Debug, Copy, Clone)]
#[repr(u32)]
pub enum StructureToken {
    BeginNode = 1,
    EndNode = 2,
    Prop = 3,
    Nop = 4,
    End = 9,
    #[num_enum(default)]
    Unknown,
}

#[repr(C)]
pub struct PropToken {
    token: u32,
    len: u32,
    name_offset: u32,
}

pub unsafe fn get_string(offset: usize) -> &'static str {
    CStr::from_ptr(
        (DEVICE_TREE_BASE as *const u8)
            .add((*DEVICE_TREE_BASE).offset_dt_strings.swap_bytes() as usize)
            .add(offset as usize),
    )
    .to_str()
    .unwrap()
}

fn build(mut token: *const u32) -> Node {
    // Stores this node's future parents
    // The parent-child relationship gets made in EndNode
    let mut node_stack: VecDeque<Node> = VecDeque::new();
    let mut current_node = None;
    loop {
        match StructureToken::from(unsafe { (*token).swap_bytes() }) {
            StructureToken::BeginNode => {
                if let Some(t) = current_node.take() {
                    node_stack.push_back(t);
                }

                // After the token, there's a null-terminated name
                let name = unsafe { CStr::from_ptr(token.add(1) as *const u8) };
                unsafe {
                    token =
                        (token as *const u8).add(name.to_bytes_with_nul().len() + 1) as *const u32
                };

                // Align to 4-bytes
                let remain = token as usize % 4;
                let mut div: usize = token as usize / 4;
                if remain != 0 {
                    div += 1;
                }
                token = (div * 4) as *const u32;

                // Create the current node
                current_node = Some(Node::new(name.to_str().unwrap()));
            }
            StructureToken::EndNode => {
                if let Some(t) = current_node.take() {
                    if let Some(mut last) = node_stack.pop_back() {
                        last.insert_child(t);
                        current_node = Some(last);
                    } else {
                        current_node = Some(t)
                    }
                }

                token = unsafe { token.add(1) };
            }
            StructureToken::Prop => {
                let struc = token as *mut PropToken;
                // SAFETY: If the FDT is correct then this shouldn't fail
                let name = unsafe { get_string((*struc).name_offset.swap_bytes() as usize) };
                let len = unsafe { (*struc).len.swap_bytes() } as usize;
                token = unsafe { token.add(3) };
                if let Some(ref mut t) = current_node {
                    t.properties.insert(
                        name,
                        PropertyValue::guess(
                            unsafe { core::slice::from_raw_parts(token as *const u8, len) },
                            Some(name),
                        ),
                    );
                }

                token = unsafe { (token as *const u8).add(len) as *const u32 };

                let remain = token as usize % 4;
                let mut div: usize = token as usize / 4;
                if remain != 0 {
                    div += 1;
                }
                token = (div * 4) as *const u32;
            }
            StructureToken::Nop => {
                token = unsafe { token.add(1) };
            }
            StructureToken::End => break,
            StructureToken::Unknown => {
                // TODO fix the unknown tokens
                // unsafe { warn!("Unknown token: {}", (*(token as *const u32))) };
                token = unsafe { token.add(1) };
            }
        };
    }
    current_node.unwrap()
}

// SAFETY: Only when init() has been called
pub fn root() -> &'static RwLock<Node> {
    unsafe { DEVICE_TREE_ROOT.assume_init_ref() }
}

/// Replace PHandleRaw attributes with PHandle attributes and put references to the nodes inside of them
pub fn link_phandles() {
    // Construct a phandle map
    let mut phandles: BTreeMap<u32, &'static Node> = BTreeMap::new();
    let borrow = unsafe { DEVICE_TREE_ROOT.assume_init_mut() }.get_mut();
    borrow.walk(&mut |node: &'static Node| {
        if let Some(PropertyValue::u32(phandle)) = node.properties.get("phandle") {
            phandles.insert(*phandle, node);
        }
    });

    // Apply the map to the values
    // Here we unsafely mutably borrow the FDT
    unsafe { DEVICE_TREE_ROOT.assume_init_mut() }
        .write()
        .walk_mut(&mut |node: &mut Node| {
            for value in node.properties.values_mut() {
                if let PropertyValue::PHandleRaw(handle) = value {
                    if let Some(target_node) = phandles.get(handle) {
                        *value = PropertyValue::PHandle(target_node)
                    } else {
                        warn!("fdt: unknown phandle {}", handle)
                    }
                }
            }
        });
}

pub fn init(header_addr: *const FdtHeader) -> &'static RwLock<Node> {
    unsafe { DEVICE_TREE_BASE = header_addr };
    let token_addr = unsafe {
        (DEVICE_TREE_BASE as *const u8)
            .add((*DEVICE_TREE_BASE).offset_dt_struct.swap_bytes() as usize)
    } as *const u32;
    let root_tree = build(token_addr);
    unsafe { DEVICE_TREE_ROOT = MaybeUninit::new(RwLock::new(root_tree)) };
    link_phandles();
    root()
}

/*
pub fn get_cpu_node() -> &'static Node {
    let cpus = root().read().get("cpus").expect("No 'cpus' node in device tree!");
    &cpus.children["cpu"][&Some(unsafe { (*read_sscratch()).hartid })]
}*/
