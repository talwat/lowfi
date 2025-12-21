# lowfi

Lowfi est une petite application écrite en Rust qui sert un objectif unique : écouter de la lofi.
Elle le fait de la manière la plus simple possible : pas d’albums, pas de pubs, juste de la lofi.

![exemple image](../../media/example1.png)

## Attention

À partir de la version 1.7.0 de lowfi, **tous** les fichiers audio intégrés par défaut proviennent de [chillhop](https://chillhop.com/). 
Consultez [MUSIQUE](./MUSIQUE.md) pour plus d’informations.

## Pourquoi ?

Je déteste les plateformes de musique modernes, et je voulais une application, petite et simple, qui mettrait simplement de la lofi aléatoire, sans vidéo ni autres fioritures.

Au-delà de ça, elle a aussi été conçue pour être assez résistante aux connections instables, et *cache* 5 morceaux entiers à la fois.

## Installation

> [!NOTE]
> Si vous êtes intéressé par la maintenance d’un paquet pour `lowfi` sur des gestionnaires de paquets comme Homebrew ou autres, ouvrez une issue.

### Dépendances

Sur toutes les plateformes : Rust 1.83.0+.

Sur macOS et Windows, aucune dépendance supplémentaire n’est nécessaire.

Sur Linux, vous aurez aussi besoin d’openssl et d’alsa.

* `alsa-lib` sur Arch, `libasound2-dev` sur Ubuntu, `alsa-lib-devel` sur Fedora.
* `openssl` sur Arch, `libssl-dev` sur Ubuntu, `openssl-devel` sur Fedora.

Si vous utilisez PulseAudio vous aurez aussi besoin d’installer `pulseaudio-alsa`.

### Cargo

La méthode d’installation recommandée est cargo :

```sh
cargo install lowfi

# Si vous voulez utiliser le protocole MPRIS.
cargo install lowfi --features mpris
```

Assurez-vous que `$HOME/.cargo/bin` est ajouté à votre `$PATH`.
Voir également [Fonctionnalités supplémentaires](#fonctionnalités-supplémentaires) pour des fonctionnalités étendues.

### Packets précompilés

Si vous rencontrez des difficultés ou ne souhaitez pas utiliser cargo, vous pouvez simplement télécharger les exécutables précompilés depuis la [dernière release](https://github.com/talwat/lowfi/releases/latest).

### AUR

```sh
yay -S lowfi
```

### openSUSE

```sh
zypper install lowfi
```

### Debian

> [!NOTE]
> Ce packet est sur un dépôt non officiel maintenu par [Dario Griffo](https://github.com/dariogriffo).

```sh
curl -sS https://debian.griffo.io/3B9335DF576D3D58059C6AA50B56A1A69762E9FF.asc | gpg --dearmor --yes -o /etc/apt/trusted.gpg.d/debian.griffo.io.gpg
echo "deb https://debian.griffo.io/apt $(lsb_release -sc 2>/dev/null) main" | sudo tee /etc/apt/sources.list.d/debian.griffo.io.list
sudo apt install -y lowfi
```

### Fedora (COPR)

> [!NOTE]
> Ce packet utilise un dépôt COPR non officiel par [FurqanHun](https://github.com/FurqanHun).

```sh
sudo dnf copr enable furqanhun/lowfi
sudo dnf install lowfi
```

### Manuel

Utile pour le débogage.

```sh
git clone https://github.com/talwat/lowfi
cd lowfi

# Si vous voulez un exécutable
cargo build --release --all-features
./target/release/lowfi

# Si vous voulez juste tester
cargo run --all-features
```

## Utilisation

`lowfi`

Oui, c’est tout.

### Contrôles

| Touche             | Fonction            |
| ------------------ | ------------------- |
| `s`, `n`, `l`      | Passer le morceau   |
| `p`, Espace        | Lecture / Pause     |
| `+`, `=`, `k`, `↑` | Volume +10 %        |
| `→`                | Volume +1 %         |
| `-`, `_`, `j`, `↓` | Volume -10 %        |
| `←`                | Volume -1 %         |
| `q`, CTRL+C        | Quitter             |
| `b`                | Ajouter aux Favoris |

> [!NOTE]
> En plus de ces contrôles habituels, lowfi est compatible avec les touches multimédia de votre machine ainsi qu'avec le standard [MPRIS](https://wiki.archlinux.org/title/MPRIS) (avec des outils comme `playerctl`).
>
> MPRIS est actuellement une [fonctionnalité optionnelle](#fonctionnalités-supplémentaires) dans Cargo (activée avec `--features mpris`) car elle est uniquement destinée à Linux, et parce que le but principal de lowfi est son interface unique et minimaliste.

### Favoris

Les favoris sont la réponse extrêmement simple de lowfi à la question « et si je voulais garder un morceau ? ».
Vous pouvez ajouter ou retirer des morceaux des favoris avec `b`, et les lire avec `lowfi -t bookmarks`.

D’un point de vue technique, vos favoris ne sont pas différents de n’importe quelle autre liste de morceaux, et sont donc stockés dans le même répertoire.

### Options supplémentaires

Si vous avez quelque chose que vous souhaitez ajuster dans lowfi, vous pouvez utiliser des options supplémentaires qui modifient légèrement l’interface ou le comportement du menu.
Les options peuvent être consultées avec `lowfi --help`.

| Option                              | Fonction                                                                       |
| ----------------------------------- | ------------------------------------------------------------------------------ |
| `-a`, `--alternate`                 | Utiliser un écran de terminal alternatif                                       |
| `-m`, `--minimalist`                | Masquer la barre de contrôle inférieure                                        |
| `-b`, `--borderless`                | Exclure les bordures de l’interface                                            |
| `-p`, `--paused`                    | Lancer lowfi en pause,                                                         |
| `-f`, `--fps`                       | FPS de l’interface [défaut : 12]                                               |
| `--timeout`                         | Délai d’attente en secondes pour les téléchargements                           |
| `-d`, `--debug`                     | Inclure les logs ALSA et autres                                                |
| `-w`, `--width <WIDTH>`             | Largeur du lecteur, de 0 à 32 [défaut : 3]                                     |
| `-t`, `--track-list <TRACK_LIST>`   | Utiliser une [liste de pistes personnalisée](#listes-de-pistes-personnalisées) |
| `-s`, `--buffer-size <BUFFER_SIZE>` | Nombre de morceaux ajoutés au cache en avance [défaut : 5]                     |

### Fonctionnalités supplémentaires

lowfi utilise le système de « features » de cargo/rust pour rendre certaines parties du programme optionnelles, notamment celles qui ne sont censées être utilisées que par une minorité d’utilisateurs.

#### `scrape` - Scraping

Cette fonctionnalité fournit la commande `scrape`.
Elle n’est généralement pas très utile, mais est incluse par souci de transparence.

Plus d’informations sont disponibles en exécutant `lowfi help scrape`.

#### `mpris` - MPRIS

Active MPRIS.

#### `extra-audio-formats` - Formats audio supplémentaires

Ceci est uniquement pertinent pour les utilisateurs de listes de pistes personnalisées ; dans ce cas, cela permet plus de formats que le simple MP3, à savoir FLAC, Vorbis et WAV.

Ces formats devraient couvrir environ 99 % des fichiers audio que les gens souhaitent lire. Si vous faites partie du 1 % utilisant un autre format audio, et présent dans [cette liste](https://github.com/pdeljanov/Symphonia?tab=readme-ov-file#codecs-decoders), ouvrez une issue.

### Listes de pistes personnalisées

> [!NOTE]
> Certains gentils utilisateurs, en particulier [danielwerg](https://github.com/danielwerg), ont déjà créé des listes alternatives situées dans le dossier [data](https://github.com/talwat/lowfi/blob/main/data/) de ce dépôt. Vous pouvez les utiliser avec lowfi en utilisant l’option `--track-list`.
>
> N’hésitez pas à proposer votre propre liste via une pull request.

lowfi prend également en charge les listes de pistes personnalisées, bien que celle par défaut de chillhop soit intégrée directement dans l'exécutable.

Pour utiliser une liste personnalisée, utilisez l’option `--track-list`. Cela peut être soit un chemin vers un fichier, soit le nom d’un fichier (sans l’extension `.txt`) présent dans le dossier données.

> [!NOTE]
> Répertoires de données par plateforme :
>
> * Linux - `~/.local/share/lowfi`
> * macOS - `~/Library/Application Support/lowfi`
> * Windows - `%appdata%\Roaming\lowfi`

Par exemple, `lowfi --track-list minipop` chargera `~/.local/share/lowfi/minipop.txt`.
Tandis que `lowfi --track-list ~/Music/minipop.txt` chargera depuis le répertoire spécifié.

Tous les morceaux doivent être au format MP3, sauf si lowfi a été compilé avec la fonctionnalité `extra-audio-formats`, qui ajoute la prise en charge de certains autres formats.

#### Le format

Dans les listes, la première ligne est appelée l’en-tête, suivie du reste des pistes.
Chaque piste sera d’abord concaténée à l’en-tête, puis l’ensemble sera utilisé pour télécharger le morceau.

> [!NOTE]
> lowfi *n’ajoutera pas* de `/` entre la base et la piste pour plus de flexibilité ;
> dans la plupart des cas, vous devriez donc avoir un `/` final dans votre en-tête.

L’exception à cette règle est lorsque le nom de la piste commence par un protocole tel que `https://`, auquel cas la base ne sera pas préfixée. Si toutes vos pistes sont de ce type, vous pouvez mettre `noheader` comme première ligne et ne pas avoir d’en-tête du tout.

Par exemple, dans cette liste :

```txt
https://lofigirl.com/wp-content/uploads/
2023/06/Foudroie-Finding-The-Edge-V2.mp3
2023/04/2-In-Front-Of-Me.mp3
https://file-examples.com/storage/fe85f7a43b689349d9c8f18/2017/11/file_example_MP3_1MG.mp3
```

lowfi téléchargerait ces trois URL :

* `https://lofigirl.com/wp-content/uploads/2023/06/Foudroie-Finding-The-Edge-V2.mp3`
* `https://file-examples.com/storage/fe85f7a43b689349d9c8f18/2017/11/file_example_MP3_1MG.mp3`
* `https://lofigirl.com/wp-content/uploads/2023/04/2-In-Front-Of-Me.mp3`

De plus, vous pouvez choisir un nom d’affichage personnalisé pour une piste,
indiqué par un `!`. Par exemple, avec une entrée comme celle-ci :

```txt
2023/04/2-In-Front-Of-Me.mp3!nom personnalisé
```

lowfi téléchargera depuis la première partie et affichera la seconde comme nom du morceau.

`file://` peut être utilisé devant une piste ou un en-tête pour que lowfi le traite comme un fichier local.
C’est utile si vous souhaitez utiliser un fichier local comme URL de base, par exemple :

```txt
file:///home/utilisateur/Musique/
fichier.mp3
file:///home/utilisateur/Musique 2/deuxieme-fichier.mp3
```

D’autres exemples sont disponibles dans le dossier
[data](https://github.com/talwat/lowfi/tree/main/data).

