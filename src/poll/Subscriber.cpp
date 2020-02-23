#include <utility>
#include "Subscriber.h"
#include <cmath>

Subscriber::~Subscriber() {
	if (this->fd >= 0) {
		close(this->fd);
	}
}

int Subscriber::get_fd() const {
	return fd;
}

void Subscriber::set_fd(int mdf) {
	this->fd = mdf;
	this->dirty = true;
}

short Subscriber::get_mask() const {
	return expected;
}

void Subscriber::set_expected(short mmask) {
	this->expected = mmask;
	this->dirty = true;
}

Subscriber::Subscriber(std::string name) : dirty(false), name(name) {}

const std::string &Subscriber::get_name() const {
	return name;
}

void Subscriber::on_error(Poll &p, int event) {
	std::cout << this->name << ": on_error event_code=" << event << " errno=" << from_errno() << std::endl;
	p.unsubscribe(*this);
}

void Subscriber::on_input(Poll &p) {

}

void Subscriber::on_output(Poll &p) {

}

bool Subscriber::is_dirty() const {
	return dirty;
}

void Subscriber::set_dirty(bool dirty) {
	this->dirty = dirty;
}

void Subscriber::disable() {
	fd = -(abs(fd));
	set_dirty(true);
}

void Subscriber::enable() {
	fd = abs(fd);
	set_dirty(true);
}

