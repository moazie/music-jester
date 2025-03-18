use iced::widget::{button, Column, Container, scrollable, Text};
use iced::{Application, Command, Element, Length, Settings, Theme};
use rfd::FileDialog;
use std::fs;
use std::path::{Path, PathBuf};
use rodio::{ OutputStream, OutputStreamHandle, Sink};

pub fn main() -> iced::Result {
    let font_bytes = include_bytes!("../assets/Noto Sans CJK Regular.otf");

    MusicJester::run(Settings {
        default_font: Some(font_bytes), // Set custom font
        
        window: iced::window::Settings {
            size: (800, 600), // Set width and height (e.g., 800x600)
            resizable: true,  // Allow resizing (set to false to prevent resizing)
            ..Default::default()
        },
        ..Default::default()
    })
}

struct MusicJester {
    selected_folder: String,
    audio_files: Vec<PathBuf>,
    scan_status: String,
    // Hold the output stream and its handle to ensure playback lives.
    playing_stream: Option<(OutputStream, OutputStreamHandle)>,
    // A sink to control playback (pause, resume, stop).
    sink: Option<Sink>,
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
                    // Automatically scan after a folder is selected.
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
    // Stop any existing playback.
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
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        // Button to select a folder.
        let folder_button = button("Select Folder").on_press(Message::FolderButtonPressed);

        let folder_display = Text::new(if self.selected_folder.is_empty() {
            "No folder selected".to_string()
        } else {
            format!("Selected folder: {}", self.selected_folder)
        });

        let status_text = Text::new(&self.scan_status);

        // List audio files as clickable buttons.
        let files_list = if self.audio_files.is_empty() {
            Column::new().push(Text::new("No audio files found yet"))
        } else {
            let mut col = Column::new().spacing(5);
            for file in &self.audio_files {
                if let Some(filename) = file.file_name() {
                    if let Some(name) = filename.to_str() {
                        col = col.push(
                            button(name)
                                .on_press(Message::PlayAudio(file.clone()))
                                .padding(5),
                        );
                    }
                }
            }
            col
        };

        let files_scrollable = scrollable(
            Container::new(files_list)
                .width(Length::Fill)
                .padding(10),
        )
        .height(Length::Fill);

        // Playback control buttons (only visible when an audio is playing).
        let controls = if self.sink.is_some() {
            Column::new()
                .spacing(10)
                .push(button("Pause").on_press(Message::PausePlayback))
                .push(button("Resume").on_press(Message::ResumePlayback))
                .push(button("Stop").on_press(Message::StopPlayback))
        } else {
            Column::new()
        };

        // Build the UI column.
        Column::new()
            .padding(20)
            .spacing(10)
            .push(folder_button)
            .push(folder_display)
            .push(status_text)
            .push(files_scrollable)
            .push(controls)
            .into()
    }
}

// Recursively finds audio files in the given directory.
fn find_audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut audio_files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let mut subdir_files = find_audio_files(&path);
                audio_files.append(&mut subdir_files);
            } else if is_supported_audio_file(&path) {
                audio_files.push(path);
            }
        }
    }
    audio_files
}

// Checks if a file is a supported audio format.
fn is_supported_audio_file(path: &Path) -> bool {
    if let Some(extension) = path.extension() {
        if let Some(ext_str) = extension.to_str() {
            return matches!(
                ext_str.to_lowercase().as_str(),
                "mp3" | "wav" | "ogg" | "flac" | "mp4" | "m4a" | "aac"
            );
        }
    }
    false
}