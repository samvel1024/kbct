#include <iostream>
#include "KeyListener.h"
#include "KeyMapper.h"
#include "UInput.h"
#include "poll/Poll.h"
#include "poll/KillReceiver.h"
#include "DeviceStateListener.h"
#include "KeyboardGrabManager.h"
#include <libevdev-1.0/libevdev/libevdev.h>

void usage() {
	std::cerr << "Usage:\nkeylaymap grab <json_path>\nkeylaymap list";
	exit(1);
}

int main(int argc, char **argv) {
	if (argc == 2 && std::string(argv[1]) == "list") {
		auto vec = KeyboardGrabManager::get_keyboard_devices();
		for (auto const &v: vec) {
			std::cout << v << std::endl;
		}
	} else if (argc == 3 && std::string(argv[1]) == "grab") {
		std::string json = argv[2];
		KeyboardGrabManager manager(KeymapConfig::parse_from_json_file(json));
		manager.listen();
	} else usage();
	return 0;
}