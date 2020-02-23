## Layered Keycode Mapping

This is simple keycode mapping utility. It performs the keycode mapping in low level *(by grabbing the keyboard chardriver and redirecting the mapped stream to uinput)* so it's not dependent on window managers like Xorg and is independent of input source language you are using. 

#### Why not **xmodmap** or **xkb** ?

If you have ever tried to have a universal ALT + IJKL or HJKL to arrow keys mapping, you have probably failed because the mapping got reset when changing input language or not all applications did understand the mapped arrow keys (like IntelliJ). Let alone the fact that it's almost impossible to compose mapped keys with other modifiers (like expecting LEFT_CTRL + ALT + I to be the same as LEFT_CTRL + UP)

### Building and running

To build the program from sources run 
```
git clone https://github.com/samvel1024/laykeymap && \
cd laykeymap && \
./build
```

1. First configure your keymap following the examples below.
The identifiers are taken from the standard [linux keycode names](https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h). 
Remember that layer modifiers are ignored completely.
Place this in a file named laykeymap.json
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
  }  
}
```
2. Find the keyboard char driver location. In other words run  `cat /proc/bus/input/devices`. From the list identify your keyboard. In my example I need to use  event11
```
(sample content in /proc/bus/input/devices)
I: Bus=0005 Vendor=17ef Product=6048 Version=0312
N: Name="ThinkPad Compact Bluetooth Keyboard with TrackPoint"
P: Phys=....
S: Sysfs=....
H: Handlers=sysrq rfkill kbd mouse4 event11 js0 leds 
B: PROP=21
... Not important
``` 

3. To execute the keymapper just run
```
sudo ./keylaymap /dev/input/event11 laykeymap.json
```

