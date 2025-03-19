use iced::widget::{button, scrollable, Column, Container, Row, Text, image};
use iced::{Application, Command, Element, Length, Settings, Theme};
use rfd::FileDialog;
use std::fs;
use std::path::{Path, PathBuf};
use rodio::{OutputStream, OutputStreamHandle, Sink};
use lofty::{Accessor, TaggedFileExt};

pub fn main() -> iced::Result {
    let font_bytes = include_bytes!("../assets/Noto Sans CJK Regular.otf");

    MusicJester::run(Settings {
        default_font: Some(font_bytes),
        window: iced::window::Settings {
            size: (800, 600),
            resizable: true,
            ..Default::default()
        },
        ..Default::default()
    })
}

struct MusicJester {
    selected_folder: String,
    audio_files: Vec<PathBuf>,
    scan_status: String,
    playing_stream: Option<(OutputStream, OutputStreamHandle)>,
    sink: Option<Sink>,
    album_art: Option<Vec<u8>>, // Store album art
    song_title: Option<String>, // Store song title
    artist: Option<String>,     // Store artist
}

#[derive(Debug, Clone)]
enum Message {
    FolderButtonPressed,
    FolderSelected(Option<String>),
    ScanComplete(Vec<PathBuf>),
    PlayAudio(PathBuf),
    PausePlayback,
    ResumePlayback,
    StopPlayback,
    DisplayAlbumArtAndMetadata(Option<Vec<u8>>, Option<String>, Option<String>), // New message
}

impl Application for MusicJester {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self {
                selected_folder: String::new(),
                audio_files: Vec::new(),
                scan_status: String::new(),
                playing_stream: None,
                sink: None,
                album_art: None,
                song_title: None,
                artist: None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Music Jester")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::FolderButtonPressed => {
                Command::perform(
                    async {
                        let result = FileDialog::new().pick_folder();
                        result.map(|path| path.display().to_string())
                    },
                    Message::FolderSelected,
                )
            }
            Message::FolderSelected(maybe_path) => {
                if let Some(path) = maybe_path {
                    self.selected_folder = path;
                    self.audio_files.clear();
                    self.scan_status = "Scanning...".to_string();
                    let folder_path = self.selected_folder.clone();
                    return Command::perform(
                        async move { find_audio_files(Path::new(&folder_path)) },
                        Message::ScanComplete,
                    );
                }
                Command::none()
            }
            Message::ScanComplete(files) => {
                self.audio_files = files;
                self.scan_status = format!("Found {} audio files", self.audio_files.len());
                Command::none()
            }
            Message::PlayAudio(file_path) => {
                if let Some(ref sink) = self.sink {
                    sink.stop();
                }
                self.sink = None;
                self.playing_stream = None;
    
                if let Ok((stream, stream_handle)) = OutputStream::try_default() {
                    if let Ok(file) = fs::File::open(&file_path) {
                        let reader = std::io::BufReader::new(file);
                        match rodio::Decoder::new(reader) {
                            Ok(decoder) => {
                                if let Ok(sink) = Sink::try_new(&stream_handle) {
                                    sink.append(decoder);
                                    sink.play();
                                    self.sink = Some(sink);
                                    self.playing_stream = Some((stream, stream_handle));
    
                                    // Extract album art, title, and artist, then update UI
                                    let album_art = extract_album_art(&file_path);
                                    let (title, artist) = extract_metadata(&file_path);
    
                                    // Update the UI with the extracted data
                                    return Command::perform(
                                        async move { (album_art, title, artist) },
                                        |(album_art, title, artist)| Message::DisplayAlbumArtAndMetadata(album_art, title, artist),
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to decode the audio file: {:?}", e);
                            }
                        }
                    } else {
                        eprintln!("Failed to open the audio file");
                    }
                }
                Command::none()
            }
            Message::DisplayAlbumArtAndMetadata(Some(album_art), Some(title), Some(artist)) => {
                self.album_art = Some(album_art);
                self.song_title = Some(title);
                self.artist = Some(artist);
                Command::none()
            }
            Message::DisplayAlbumArtAndMetadata(_, _, _) => {
                // Handle the case where album art, title, or artist is None
                self.album_art = None;
                self.song_title = None;
                self.artist = None;
                Command::none()
            }
            Message::PausePlayback => {
                if let Some(sink) = &self.sink {
                    sink.pause();
                }
                Command::none()
            }
            Message::ResumePlayback => {
                if let Some(sink) = &self.sink {
                    sink.play();
                }
                Command::none()
            }
            Message::StopPlayback => {
                if let Some(sink) = &self.sink {
                    sink.stop();
                }
                self.sink = None;
                self.playing_stream = None;
                self.album_art = None; // Clear album art
                self.song_title = None; // Clear song title
                self.artist = None;     // Clear artist
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let folder_button = button("Select Folder").on_press(Message::FolderButtonPressed);
        let folder_display = Text::new(if self.selected_folder.is_empty() {
            "No folder selected".to_string()
        } else {
            format!("Selected folder: {}", self.selected_folder)
        });
        let status_text = Text::new(&self.scan_status);
    
        let files_list = if self.audio_files.is_empty() {
            Column::new().push(Text::new("No audio files found yet"))
        } else {
            let mut col = Column::new().spacing(5);
            for file in &self.audio_files {
                if let Some(filename) = file.file_name().and_then(|name| name.to_str()) {
                    col = col.push(button(filename).on_press(Message::PlayAudio(file.clone())).padding(5));
                }
            }
            col
        };
    
        let files_scrollable = scrollable(Container::new(files_list).width(Length::Fill).padding(10))
            .height(Length::Fill);
    
        let left_column = Column::new()
            .spacing(10)
            .push(folder_button)
            .push(folder_display)
            .push(status_text)
            .push(files_scrollable)
            .width(Length::FillPortion(1));
    
        // Place album art above the controls
        let album_art_view = if let Some(ref bytes) = self.album_art {
            let handle = image::Handle::from_memory(bytes.clone());
            image(handle).width(Length::Fixed(270.0)).height(Length::Fixed(270.0))
        } else {
            // Load fallback image
            let fallback_bytes = include_bytes!("../assets/fallback_image.png").to_vec();
            let handle = image::Handle::from_memory(fallback_bytes);
            image(handle).width(Length::Fixed(270.0)).height(Length::Fixed(270.0))
        };

        // Display song title and artist if available
        let song_info = if let (Some(title), Some(artist)) = (self.song_title.clone(), self.artist.clone()) {
            Column::new()
                .spacing(5)
                .push(Text::new(format!("Title: {}", title)))
                .push(Text::new(format!("Artist: {}", artist)))
        } else {
            Column::new().push(Text::new("No metadata available"))
        };
    
        // Modify the controls to be in a horizontal row
        let controls = if self.sink.is_some() {
            Row::new()
                .spacing(10)
                .push(button("Pause").on_press(Message::PausePlayback))
                .push(button("Resume").on_press(Message::ResumePlayback))
                .push(button("Stop").on_press(Message::StopPlayback))
        } else {
            Row::new().push(Text::new("No audio playing"))
        };
    
        let right_column = Column::new()
            .spacing(10)
            .push(album_art_view)  // Place album art above the controls
            .push(song_info)       // Add song info below the album art
            .push(Text::new("Playback Controls"))
            .push(controls)
            .width(Length::FillPortion(1));
    
        Row::new()
            .spacing(20)
            .push(left_column)
            .push(right_column)
            .padding(20)
            .into()
    }
}

fn find_audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut audio_files = Vec::new();
    if dir.is_dir() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Recurse into subfolders
                    audio_files.extend(find_audio_files(&path));
                } else if path.is_file() && is_supported_audio_file(&path) {
                    // Add file if it's a supported audio file
                    audio_files.push(path);
                }
            }
        }
    }
    audio_files
}

fn is_supported_audio_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()), Some("mp3" | "m4a" | "flac" | "wav" | "ogg"))
}

fn extract_album_art(file_path: &PathBuf) -> Option<Vec<u8>> {
    lofty::read_from_path(file_path).ok()?.primary_tag()?.pictures().first().map(|p| p.data().to_vec())
}

fn extract_metadata(file_path: &PathBuf) -> (Option<String>, Option<String>) {
    if let Ok(file) = lofty::read_from_path(file_path) {
        if let Some(tag) = file.primary_tag() {
            let title = tag.title().map(|s| s.to_string());
            let artist = tag.artist().map(|s| s.to_string());
            return (title, artist);
        }
    }
    (None, None)
}
