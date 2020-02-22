//
// Created by sme on 22/02/2020.
//

#ifndef KBPLUSPLUS_UINPUT_H
#define KBPLUSPLUS_UINPUT_H

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <linux/input.h>
#include <linux/uinput.h>
#include <sys/types.h>
#include <unistd.h>
#include <cstring>
#include <map>
#include <iostream>
#include <vector>

class UInput {

	int uinp_fd;

	static void do_ioctl(int fd, int p1, int p2) {
		if (ioctl(fd, p1, p2) < 0) {
			std::cerr << "Unable to perform do_ioctl on /dev/uinput" << std::endl;
			throw std::exception();
		}
	}

public:


	UInput() {
		uinp_fd = open("/dev/uinput", O_WRONLY | O_NONBLOCK);
		if (uinp_fd < 0) {
			std::cerr << "Unable to open /dev/uinput" << std::endl;
			throw std::exception();
		}

		do_ioctl(uinp_fd, UI_SET_EVBIT, EV_KEY);
		do_ioctl(uinp_fd, UI_SET_EVBIT, EV_REL);
		do_ioctl(uinp_fd, UI_SET_RELBIT, REL_X);
		do_ioctl(uinp_fd, UI_SET_RELBIT, REL_Y);
		for (int i = 0; i < 256; i++) {
			do_ioctl(uinp_fd, UI_SET_KEYBIT, i);
		}

		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_MOUSE);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_LEFT);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_MIDDLE);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_RIGHT);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_FORWARD);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_BACK);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_WHEEL);
		do_ioctl(uinp_fd, UI_SET_EVBIT, EV_SYN);
		do_ioctl(uinp_fd, UI_SET_EVBIT, EV_REL);
		do_ioctl(uinp_fd, UI_SET_RELBIT, REL_X);
		do_ioctl(uinp_fd, UI_SET_RELBIT, REL_Y);
		do_ioctl(uinp_fd, UI_SET_RELBIT, REL_WHEEL);
		do_ioctl(uinp_fd, UI_SET_EVBIT, EV_KEY);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_MOUSE);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_TOUCH);
		do_ioctl(uinp_fd, UI_SET_KEYBIT, BTN_TOOL_DOUBLETAP);

		struct uinput_user_dev uinp{};
		struct input_event event{};
		memset(&uinp, 0, sizeof(uinp));
		snprintf(uinp.name, UINPUT_MAX_NAME_SIZE, "uinput-sample");
		uinp.id.bustype = BUS_USB;
		uinp.id.vendor = 0x1;
		uinp.id.product = 0x1;
		uinp.id.version = 1;

		if (write(uinp_fd, &uinp, sizeof(uinp)) < 0) {
			std::cerr << "Unable to write UINPUT device." << std::endl;
			throw std::exception();
		}

		if (ioctl(uinp_fd, UI_DEV_CREATE) < 0) {
			std::cerr << "Unable to create UINPUT device." << std::endl;
			throw std::exception();
		}

	}

	virtual ~UInput() {
		if (uinp_fd > 0) {
			close(uinp_fd);
		}
	}

	void consume(void *ptr, int size) {
		write(uinp_fd, ptr, size);
	}

};

#endif //KBPLUSPLUS_UINPUT_H
