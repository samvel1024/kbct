## Layered Keycode Mapping

This is simple keycode mapping utility. It performs the keycode mapping in low level *(by grabbing the keyboard chardriver and redirecting the mapped stream to uinput)* so it's not dependent on window managers like Xorg and is independent of input source language you are using. 

#### Why not **xmodmap** or **xkb** ?

If you have ever tried to have a universal ALT + IJKL or HJKL to arrow keys mapping, you have probably failed because the mapping got reset when changing input language or not all applications did understand the mapped arrow keys (like IntelliJ). Let alone the fact that it's almost impossible to compose mapped keys with other modifiers (like expecting LEFT_CTRL + ALT + I to be the same as LEFT_CTRL + UP)

### Installation

1. To install the program run
```
wget https://raw.githubusercontent.com/samvel1024/laykeymap/master/scripts/install.sh && bash install.sh
```

2. Run `sudo laykeymap list` and figure out to which keyboard you want to apply the mapping. Copy the name of that keyboard (case, space sensitive).

3. Configure your keymap at /etc/laykeymap following the example below.
The identifiers are taken from the standard [linux keycode names](https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h). 
Remember that layer modifiers are ignored completely (as if they were not pressed). Paste the keyboad name in keyBoardNames array.

 ```
 {  
  "map": {  
	"KEY_LEFTALT": "KEY_LEFTCTRL",  
	"KEY_CAPSLOCK": "KEY_LEFTALT",  
    "KEY_LEFTCTRL": "KEY_RIGHTALT",  
    "KEY_SYSRQ": "KEY_RIGHTCTRL"  
  },  
  "layers": {  
    "KEY_RIGHTALT": {  
      "KEY_L": "KEY_RIGHT",  
	  "KEY_I": "KEY_UP",  
	  "KEY_J": "KEY_LEFT",  
	  "KEY_K": "KEY_DOWN",  
	  "KEY_U": "KEY_PAGEUP",  
	  "KEY_O": "KEY_PAGEDOWN",  
	  "KEY_SEMICOLON": "KEY_BACKSPACE",  
	  "KEY_APOSTROPHE": "KEY_DELETE",  
	  "KEY_N": "KEY_INSERT",  
	  "KEY_EQUAL": "KEY_BRIGHTNESSUP",  
	  "KEY_MINUS": "KEY_BRIGHTNESSDOWN",  
	  "KEY_0": "KEY_VOLUMEUP",  
	  "KEY_9": "KEY_VOLUMEDOWN",  
	  "KEY_8": "KEY_MUTE"  
    }  
  },
  "keyboardNames": ["AT Translated Set 2 keyboard"]
}
```
4. Test the mapping by `laykeymap grab /etc/laykeymap`. If you are happy with the resulting behavior enable the systemd service to run it automatically on login. Don't enable the service before testing as it might block all incoming keystrokes if not configured properly.
  
```
sudo systemctl enable laykeymap
sudo systemctl start laykeymap
```

