# The State of lowfi's Music

[[version française](../fr/MUSIQUE.md)]

> [!WARNING]
> This document will be a bit long and has almost nothing to do with the actual
> usage of lowfi, just the music embedded by default.

Before that though, some context. lowfi includes an extensive track list
embedded into the software, so you can download it and have it "just work"
out of the box.

I always hated apps that required extensive configuration just to be usable.
Sometimes it's justified, but often, it's just pointless when most will end up
with the same set of "defaults" that aren't really defaults.

lowfi is so nice and simple because of the "plug and play" aspect,
but it's become a lot harder to continue it as of late.

## The Lofi Girl List

Originally, it was planned that lowfi would use music scraped from Lofi Girl's own
website. The scraper actually came before the rest of the program, believe it or not.

However, after a long period of downtime, the Lofi Girl website was redone without the
mp3 track files. Those are now pretty much inaccessible aside from paying for individual
albums on bandcamp which gets very expensive very quickly.

Doing this was never actually disallowed, but it is now simply impossible. So, the question was,
what to do next after losing lowfi's primary source of music?

## Tracklists

I was originally against the idea of custom tracklists, because of my almost purist
ideals of a 100% no config at all vision for lowfi. But eventually, I gave in, which proved
to be a very good decision in hindsight. Now, regardless of what choices I make on the music
which is embedded, all may opt out of that and choose whatever they like.

This culminated in a few templates located in the `data` directory of this repository
which included a handful of tracklists, and in particular, the chillhop list by user
[danielwerg](https://github.com/danielwerg).

## The Switch

After `lofigirl.com` went down, I thought a bit and eventually decided
to just bite the bullet and switch to the chillhop list. This was despite the fact
that chillhop entirely bans third party players in their TOS. They also ban
scrapers, which I only learned after writing one.

So, is lowfi really going to have to violate the TOS of it's own music provider?
Well, yes. I thought about it, and came to the conclusion that lowfi is probably
not much of a threat for a few reasons.

Firstly, it emulates exactly the behavior of chillhop's own radio player.
The only difference is that one shoves you into a web browser, and the other,
into a nice terminal window.

Then, I also realize that lowfi is just a small program used by few.
I'm not making money on any of this, and I think degrading the experience for my
fellow nerds who just want to listen to some lowfi without all the crap is not worth it.

At the end of the day, lowfi has a distinct UserAgent. Should chillhop ever take issue with
it's behavior, banning it is extremely simple. I don't want that to happen, but I
understand if it does.

## Well, *I* Hate the Chillhop Music

It's not as "lofi". It is almost certainly a compromise, that much I cannot even pretend to
deny. I find myself hitting the skip button almost three times as often with chillhop.

If you are undeterred enough by TOS's to read this far, then you can use the `archive.txt`
list in the `data` folder. The list is a product of me worrying that the tracks on `lofigirl.com`
could've possibly been lost somehow, relating to the website going down.

It's hosted on `archive.org`, and could be taken down at any point for any reason.
Being derived from my own local archive, it retains ~2700 out of the ~3700 tracks.
That's not perfect, the organization is also *bad*, but it exists.
