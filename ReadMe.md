# Akashic Records
### (of vtubers)

## Description

For a number of reasons, the most important one being copyright concerns, video streams (with a particular concern for vtubers) are occasionally delisted or otherwise un-archived. This project is the Rust port of a (unreleased) Python project to automatically record vtuber streams in order to have a local copy in the case of such an event. The actual process of recording a stream is neatly taken care of by yt-dlp, the more advanced fork of the functionally defunt youtube-dl. However, this requires, at a minimum, to know about a given stream to record. For this, the HoloDex API is used to determine upcoming stream (with additional functionality). Ultimately, this project revolves around processing and managing these two things. Note that the streams are recorded in a relatively uncompressed state, and thus take up quite a bit of space (the typical stream is roughly 2GB/hour for youtube, twitch is often higher). Post-processing or format selection could reduce that, but I like having the "purest" form in storage. Secondary note: occasionally, under certain circumstances, frames or even entire moments are recorded that aren't captured in the official VoD.

Currently seems to be working fine, but needs testing after the large update.

Realistically, this is a personal project and I don't expect anyone else to actually use this. If you *do* use this, feel free to let me know, or give suggestions. So named by my roommate because I seem to record everything, though of course that is not actually the case.

### Motivations

As it currently is, this project is still missing many features the previous (Python based) version has, most of which are planned to be added incrementally in the future. There were a number of motivations in the creation of this version, in no particular order:

- The old Python project is subject to a bug in a number of versions of the Python language itself. While at some level this is easy to work around, it's a bit inconvenient when mostly ran on Raspberry Pis.

- The old Python project would benefit from some refactoring, and updates to some of the libraries that were in use broke functionality. Rust has direct replacements for some of the libraries formerly used, and appealing alternatives to others. However, it is perhaps worth noting that yt-dlp, written in Python and the primary reason Python was used in the original version, has no real equal in any language.

- Rust has caught my eye as an interesting language and this is a good opportunity to work with the language to better learn it. As an accidental sideeffect of incorporating the python based yt-dlp via pyo3, I understand both Rust and Python better than I did before I started.

- Properties of the Rust language make it appealing in terms of performance, particularly compared to Python.

## Requirements

TODO: Determine what is required to run this on different devices/platforms.

In addition to the required packages, specific files, formatted in a particular way, are required:

- "res/keys/holodex_Key.txt" A valid HoloDex API key, with no other characters in the file. As stands, this is the most essential of these files.

- "res/lists/archive_list.txt": A list of channel-ids, each on a new line and with no other characters, from which to always record. Lines starting with "#" are ignored to allow labels, sections, notes, et cetera. Requires the actual channel-ids, not the (usually named) channel references that a channel can choose. *Id est*, "UCP4nMSTdwU1KqYWu3UH5DHQ", not "@PomuRainpuff". This could be blank if all streams to record are to be found via keywords.

- "res/lists/check_list.txt": A list of channel-ids, each on a new line and with no other characters, from which to check on each API call. Lines starting with "#" are ignored to allow labels, sections, notes, et cetera. Requires the actual channel-ids, not the (usually named) channel references that a channel can choose. *Id est*, "UCP4nMSTdwU1KqYWu3UH5DHQ", not "@PomuRainpuff". This could be blank if instead all upcoming streams were checked.

- "res/lists/key_words.txt": A list of keywords to look for in the titles of upcoming videos, separated by new lines. This could be blank if only the archive list is to be used. *Note*: for logistical reasons, titles are made lowercase and stripped of whitespaces before they are checked for keywords. Rust's to_lowercase() method uses Unicode properties, meaning there is functionality beyond the ASCII characters. However, there are limits to this, and it shouldn't be expected to catch *similar* characters.

- "res/cookies.txt": A netscape structured cookie file, as described in the yt-dlp README.md. Only needed for authenticating membership streams. Note: these seem to expire very quickly, so unfortunately this doesn't seem very useful.
