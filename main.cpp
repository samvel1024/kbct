#include <iostream>
#include "KeyListener.h"
#include "KeyMapper.h"
#include "UInput.h"

int main(int argc, char **argv) {
	if (argc != 3) {
		std::cerr << "Usage: simkey <device_path> <json_path>";
		return 1;
	}
	std::string device = argv[1];
	std::string json = argv[2];
	UInput uinput;
	KeyMapper mapper = KeyMapper::configure_from_json(json, [&uinput](auto p, auto l) {
		uinput.consume(p, l);
	});
	KeyListener listener(device, [&mapper](auto &ev) {
		mapper.on_keystroke(ev);
	});
	listener.listen_keystrokes();
}