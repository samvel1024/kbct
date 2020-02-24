#ifndef LAYKEYMAP_KEYMAPPER_H
#define LAYKEYMAP_KEYMAPPER_H

#include <unordered_map>
#include <fstream>
#include <utility>
#include "keyboard.h"
#include "KeymapConfig.h"

using json = nlohmann::json;


class KeyMapper {
private:
	template<class K, class V>
	using map_t = std::unordered_map<K, V>;
	using keycode_t = unsigned int;
	using keycodemap_t = std::vector<keycode_t>;
	using keynamemap_t = map_t<std::string, std::string>;
	using consumer_t = std::function<void(void *, int)>;

	static constexpr keycode_t IGNORED = 0;


	static map_t<std::string, keycode_t> get_key_to_code() {
		map_t<std::string, keycode_t> codes{};
		for (unsigned i = 0; i < LAYKEYMAP_MAX_KEYCODE; ++i) {
			std::string val = laykeymap_key_name(i);
			if (val != "?") {
				codes[val] = i;
			}
		}
		return codes;
	}

	static void assert_key_exists(const map_t<std::string, keycode_t> &code, const std::string &str) {
		if (code.find(str) == code.end()) {
			std::cerr << "Unknown key " << str << std::endl;
			throw std::exception();
		}
	}

	static void
	assert_no_unknown_key(const map_t<std::string, keycode_t> &codes, const map_t<std::string, std::string> &keymap) {
		for (auto const &entry: keymap) {
			assert_key_exists(codes, entry.first);
			assert_key_exists(codes, entry.second);
		}
	}

	static keycodemap_t identity_keymap() {
		keycodemap_t vec(LAYKEYMAP_MAX_KEYCODE + 1, 0);
		for (keycode_t i = 0; i <= LAYKEYMAP_MAX_KEYCODE; ++i) {
			vec[i] = i;
		}
		return vec;
	}

	static keycodemap_t
	from_keynamemap(map_t<std::string, keycode_t> &available_keys, const keynamemap_t &knmap) {
		keycodemap_t kcmap = identity_keymap();
		for (auto const &v : knmap) {
			keycode_t from = available_keys[v.first];
			keycode_t to = available_keys[v.second];
			kcmap[from] = to;
		}
		return kcmap;
	}

	// The default layer is mapped under 0
	map_t<keycode_t, std::vector<keycode_t>> layers;
	std::vector<keycode_t> pressed_layer;
	keycode_t current_layer = 0;
	consumer_t mapped_event_consumer;

public:


	KeyMapper(KeymapConfig &conf, consumer_t consumer) : mapped_event_consumer(std::move(consumer)) {
		map_t<std::string, keycode_t> available_keys = get_key_to_code();
		assert_no_unknown_key(available_keys, conf.map);
		for (auto const &v: conf.layers) {
			assert_key_exists(available_keys, v.first);
			assert_no_unknown_key(available_keys, v.second);
		}

		pressed_layer = std::vector<keycode_t>(LAYKEYMAP_MAX_KEYCODE + 1, 0);
		keycodemap_t base_keycodemap = from_keynamemap(available_keys, conf.map);
		// Ignore layer modifiers
		for (auto const &v: conf.layers) {
			keycode_t layer_modifier = available_keys[v.first];
			base_keycodemap[layer_modifier] = IGNORED;
			layers[layer_modifier] = from_keynamemap(available_keys, v.second);
		}
		layers[0] = base_keycodemap;
	}

	// Returning false means the keystroke has to be escaped (not redirected to target output stream)
	inline bool map_keystroke(std::vector<struct input_event> &vec, int from, int to) {
		for (auto &ev: vec) {
			if (ev.type != EV_KEY)
				continue;

			keycode_t key = ev.code;
			bool released = ev.value == 0;
			bool pressed = !released;

			if (released && current_layer == key) {
				current_layer = 0;
				return false;
			}

			if (pressed && layers[current_layer][key] == 0) {
				current_layer = key;
				return false;
			}

			if (pressed) {
				pressed_layer[key] = current_layer;
			}

			if (released && pressed_layer[key] != current_layer) {
				ev.code = layers[pressed_layer[key]][key];
			} else {
				ev.code = layers[current_layer][key];
			}
		}
		return true;
	}

	void on_keystroke(std::vector<struct input_event> &vec) {
		int from = 0;
		for (int i = 0; i < vec.size(); ++i) {
			struct input_event &ev = vec[i];
			if (ev.type == EV_SYN) {
				if (map_keystroke(vec, from, i + 1)) {
					int len = i - from + 1;
					mapped_event_consumer(&vec[from], len * sizeof(struct input_event));
				}
				from = i + 1;
			}
		}

	}

};

#endif //LAYKEYMAP_KEYMAPPER_H
