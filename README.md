## Layered Keycode Mapping

This is simple keycode mapping utility. It performs the keycode mapping in low level *(by grabbing the keyboard chardriver and redirecting the mapped stream to uinput)* so it's not dependent on window managers like Xorg and is independent of input source language you are using. 

#### Why not **xmodmap** or **xkb** ?
If you have ever tried to have a universal ALT + IJKL (or HJKL) to arrow keys mapping, you probably suffered because the mapping got reset when changing input language or not all applications did understand the mapped arrow keys (like IntelliJ). Let alone the fact that it's almost impossible to compose mapped keys with other modifier (like expecting LEFT_CTRL + ALT + I to be the same as LEFT_CTRL + UP)

