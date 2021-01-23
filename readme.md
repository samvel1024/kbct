## 

## WORK IN PROGRESS

## KBCT - Keyboard Customization Tool for Linux :gear: :computer: :penguin:

![img](https://i.imgur.com/n5Wn0YJ.jpg)

KBCT is yet another tool that will help to remap keys across the desktop environment.

When is kbct useful?

- If you want to have an **ergonomic keyboard layout** (when your fingers almost never need to leave the home row keys).

- If you're an ex MacOS user and want something similar to [Karabiner Elements](https://github.com/pqrs-org/Karabiner-Elements).

- If you want to have system-wide vim-like navigation mapping `some_modifier + hjkl` to arrow keys.

- If you find `xbindkeys` ,`xmodmap` and `setxkbmap` ~~impossible~~ hard to configure.

- If you want your mapping configuration to work on **both Wayland and X11**.

- If you want the configuration to be simple and intuitive.

***However, Kbct is not** a tool that can be used to configure macros or arbitrary command execution on a key press. Also note that **kbct requires sudo access**.

### 

### Installation

There are two options for installing kbct

- Download the pre-built x86_64 AppImage binary from [releases](https://github.com/samvel1024/kbct/releases).

- Compile from the sources by first installing `libudev1` package (available for all known distributions).
  
  ```
  sudo apt install libudev1
  ```
  
  Then assuming that you have a [Rust toolchain](https://www.rust-lang.org/tools/install) in place run the following.
  
  ```bash
  cd /tmp && 
  git clone https://github.com/samvel1024/kbct && \
  cd kbct && \
  cargo build --release && \
  ./target/release/kbct --help
  ```

### 

### Configuration

Kbct uses yaml files as configuration. It allows to apply different mapping rules for different keyboards. There are two main types of key mappings

- `simple`: maps keys 1-1 regardless of any modifiers. (e.g `capslock -> leftctrl`)

- `complex`: maps keys based on the active layer. Layer is a key map that will activate and override the existing mapping if a given set of keys are pressed. Much like `fn` key is combined with `F1-F12` keys. (e.g `rightalt+i=up` or `rightalt+leftctrl+comma=volumeup` )

**The following is an exhaustive example configuration of kbct**

```yaml
# Declares set of mapping rules named "main"
main: 
  # A regex selecting the keyboards that need to be mapped 
  keyboard: "(Thinkpad.*|AT Translated Set 2 keyboard)"
  # Specifiy one-to-one key mappings
  simple:
    leftalt: leftctrl
    capslock: leftalt
    sysrq: rightmeta
  # Specify layered configurations (much similar to fn+F keys)
  complex:
    # Specify the modifiers of the layer
    - modifiers: ['rightalt']
      keymap:
        i: up
        j: left
        k: down
        l: right
        u: pageup
        o: pagedown
        p: home
        semicolon: end
```

[Here](https://github.com/samvel1024/kbct/blob/master/Cargo.lock) you can find all the available key names to use in the configuration. Essentially those are taken from Linux API [headers](https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h).

In order to list all the available keyboard devices and their respective names run the following.

```bash
sudo kbct list-devices
```

You can use those names to create a regex matcher for the `keyboard` field in the configuration

**Important note:** kbct is treating `leftshift`/`rightshift` , `leftalt`/`rightalt`, etc. as different keys, so if you want to map both you need to define the mapping twice. This is done to avoid 

### How it works

KBCT is operating on a low enough level to be independent from the window manager or the desktop environment. It is achieved by the following steps

Since kbct should be run as root it has enough privileges to  read and grab the output of the keyboard (e.g the output of `/dev/input/event2`). Which means that it becomes readable only for kbct and the display manager is no longer able to read from the keyboard device.

Then kbct creates another "virtual" `uinput`device (e.g. `/dev/input/event6`), and sends customized key events to that device. The new mapped keyboard is successfully read by the window manager, which as a result reads from customized output.

You can use `evtest` to monitor the output of the kbct-mapped virtual device by this command.

```bash
sudo kbct list-devices | grep -i kbct | awk '{ print $1 }' | sudo xargs evtest 
```










