#ifndef LAYKEYMAP_DEVICESTATELISTENER_H
#define LAYKEYMAP_DEVICESTATELISTENER_H

#include "poll/Subscriber.h"
#include <string>
#include <utility>
#include <sys/inotify.h>

class DeviceStateListener : public Subscriber {

	int watched_dir;
	std::function<void(void)> on_refresh_devices;
	std::vector<char> buffer = std::vector<char>(1024 * sizeof(struct inotify_event));

public:

	DeviceStateListener(std::function<void(void)> on_refresh)
			: Subscriber(std::string("device-listener")), on_refresh_devices(std::move(on_refresh)) {
		int fd = inotify_init();
		watched_dir = inotify_add_watch(fd, "/dev/input", IN_CREATE | IN_DELETE);
		set_fd(fd);
		set_expected(POLLIN | POLLERR | POLLHUP);
	}

	virtual void on_error(Poll &p, int event) {
	}

	virtual void on_input(Poll &p) {

		int read_len = read(fd, buffer.data(), buffer.size());

		/*checking for error*/
		if (read_len < 0) {
			std::cerr << "Error in DeviceStateListener read" << std::endl;
			return;
		}

		auto *events = reinterpret_cast<inotify_event *>(buffer.data());
		int max_ev = buffer.size() / sizeof(struct inotify_event);
		for (int i = 0; i < max_ev; ++i) {
			inotify_event *event = &events[i];
			if (event->len) {
				on_refresh_devices();
			}
		}
	}

	virtual void on_output(Poll &p) {
	}

	virtual ~DeviceStateListener() {
		if (watched_dir > 0) {
			inotify_rm_watch(this->fd, watched_dir);
			watched_dir = 0;
		}
	}
};

#endif //LAYKEYMAP_DEVICESTATELISTENER_H
