## 

## KBCT - Keyboard Customization Tool for Linux :gear: :computer: :penguin:

![img](https://i.imgur.com/ryVuxe5.jpeg)

KBCT is yet another tool that will help to remap keys across the desktop environment.

When is KBCT useful?

- If you want to have a custom **ergonomic keyboard layout** (when your fingers almost never need to leave the home row keys).

- If you're an ex MacOS user and want something similar to [Karabiner Elements](https://github.com/pqrs-org/Karabiner-Elements).

- If you want to achieve something similar to **QMK layers on your laptop keyboard**.

- If you want to have system-wide **vim-like navigation** mapping `some_modifier + hjkl` to arrow keys.

- If you find `xbindkeys` ,`xmodmap` and `setxkbmap` ~~impossible~~ hard to configure.

- If you want your mapping configuration to work on **both Wayland and X11**.

- If you want to remap buttons on your **mouse** or other input devices.

- If you want the configuration to be simple and intuitive.

***However, KBCT is not** a tool that can be used to configure macros or arbitrary command execution on a key press. Also note that **KBCT requires sudo access**.

****KBCT is in active development** so expect to see some bugs, however it should be stable enough for simple use cases. In any case create an issue if you encounter something unexpected.

### 

### Installation

There are several ways of installing KBCT

- Download the pre-built x86_64 AppImage binary from [releases](https://github.com/samvel1024/kbct/releases).

  ```bash
  cd ~/Downloads
  wget https://github.com/samvel1024/kbct/releases/latest/download/kbct-x86_64.AppImage
  chmod +x kbct-x86_64.AppImage

  #Check that it works
  sudo ./kbct-x86_64.AppImage list-devices
  ```

- Compile from the sources by first installing `libudev1` and `libudev-dev` packages (available for all known distributions).

  ```
  sudo apt install libudev1 && \
	sudo apt install libudev-dev  # for ubuntu/debian
  ```

  Then assuming that you have a [Rust toolchain](https://www.rust-lang.org/tools/install) installed run the following.

  ```bash
  cd /tmp &&
  git clone https://github.com/samvel1024/kbct && \
  cd kbct && \
  cargo build --release && \
  ./target/release/kbct --help
  ```

- Install from the AUR

  If you are an Arch Linux user, you can install it from
  [AUR](https://aur.archlinux.org/):

  ```
  $ yay -S kbct-git
  ```

  > Note: The configuration file is expected to be in
  > `/etc/kbct/config.yml`.

  After the installation, run the systemd service (remember to `modprobe uinput` first):

  ```
  $ systemctl start kbct
  ```

### 

### Configuration

KBCT uses YAML files as configuration. It allows to apply different mapping rules for different keyboards. There are two main types of key mappings

- `keymap`: maps keys 1-1 regardless of any  layer modifiers. (e.g `capslock -> leftctrl`)

- `layers`: maps keys based on the active layer. Layer is a key map that will activate and override the existing mapping if a given set of keys are pressed. Much like `fn` key is combined with `F1-F12` keys. (e.g `rightalt+i=up` or `rightalt+leftctrl+comma=volumeup` )

**The following is an exhaustive example configuration of KBCT**

```yaml
# Apply this configuration to two keyboards (if connected)
- keyboards: [ "Lenovo TrackPoint Keyboard II", "AT Translated Set 2 keyboard"]

  keymap:
    leftalt: leftctrl
    capslock: leftalt
    sysrq: rightmeta
  # Specify layered configurations (much similar to fn+F keys)
  layers:
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

As a result the above configuration will have the following effect

```textile
# ↓/↑ stand for press/release events
# One to one example
leftalt↓ ⟶ leftctrl↓
leftalt↑ ⟶ leftctrl↑

# Layer example
rightalt↓ ⟶ rightalt↓
i↓ ⟶ rightalt↑ up↓
i↑ ⟶ up↑
rightalt↑ ⟶ ∅
```

Load uinput module (**kbct will not function but will not produce an error if the uinput module is not loaded**)

```bash
sudo modprobe uinput
```

To start KBCT based on YAML configuration file run:

```bash
sudo kbct remap --config ~/.config/kbct.yaml 
```

[Here](https://gist.githubusercontent.com/samvel1024/02e5675e04f9d84f098e98bcd0e1ea12/raw/e18d950ce571b4ff5c832cc06406e9a6afece132/keynames.txt) you can find all the available key names to use in the configuration. Essentially those are taken from Linux API [headers](https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h). In case you want to disable a key map it to `reserved`. For example disabling capslock will look like this `capslock: reserved`.



**Important note:** KBCT is treating `leftshift`/`rightshift` , `leftalt`/`rightalt`, etc. as different keys, so if you want to map both you need to define the mapping twice. This is done on purpose to give fine grained control over configuration.

### Troubleshooting
**What is the name of my keyboard?**
In order to list all the available keyboard devices and their respective names run the following:

```bash
sudo kbct list-devices
```

Most often a keyboard laptop will be named `AT Translated Set 2 keyboard`. If you're not sure what the name of your keyboard is, run `sudo evtest`, select a device from a list and try typing. If it lets you type without spitting output, you selected a wrong device. Repeat until you see output like this:
```
Event: time 1641154916.130391, -------------- SYN_REPORT ------------
Event: time 1641154916.130391, type 4 (EV_MSC), code 4 (MSC_SCAN), value 7004f
Event: time 1641154916.130391, type 1 (EV_KEY), code 106 (KEY_RIGHT), value 0
Event: time 1641154916.130391, -------------- SYN_REPORT ------------
```

**What are the names of the keys?**
KBCT uses the lowest possible level keycodes from the Linux kernel to perform remapping. Window managers/desktop environments may have other namings for the same keys for various reasons. To know the exact name of the key you're interested you can use either `sudo evtest /dev/input/event<i>`, or `sudo kbct log-keys --device-path /dev/input/event<i>` where `<i>` should be replaced by the appropriate device number. You can then start typing to see the key names.

### How it works

KBCT is operating on a low enough level to be independent from the window manager or the desktop environment. It is achieved by the following steps:

Since KBCT should be run as root, it has enough privileges to read and grab the output of a keyboard or another input device (e.g the output of `/dev/input/event2`). Which means that it becomes readable only for KBCT and that the display manager is no longer able to read from that device.

Then KBCT creates another virtual `uinput`device (e.g. `/dev/input/event6`), and sends customized key events to that device. The new mapped keyboard or device is successfully read by the window manager, which as a result reads customized key events.


### Examples

See the [examples.md](./examples.md) for user-submitted kbct remap examples.
