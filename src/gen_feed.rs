use chrono::{Date, Duration, NaiveDate, Utc};
use rss::extension::itunes::{
    ITunesChannelExtensionBuilder, ITunesItemExtension, ITunesItemExtensionBuilder,
};
use rss::extension::ExtensionBuilder;
use rss::{ChannelBuilder, Enclosure, EnclosureBuilder, Guid, GuidBuilder, Item, ItemBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::default;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::time::SystemTime;
use std::{collections::BTreeMap, env, fs::File};
use uuid::Uuid;
use youtube_dl::model::SingleVideo;
use youtube_dl::Playlist;
use ytd_rs::{error::YoutubeDLError, Arg, YoutubeDL, YoutubeDLResult};

fn get_recent_videos(channel_id: String) -> Vec<SingleVideo> {
    let link = format!("https://www.youtube.com/channel/{channel_id}");

    let command = Command::new("which").arg("yt-dlp").output().unwrap();

    let ytdlp_path = std::str::from_utf8(&command.stdout)
        .expect("failed to parse the stdout of the yt-dlp command")
        .trim();

    let output = youtube_dl::YoutubeDl::new(&link)
        .youtube_dl_path(ytdlp_path)
        .socket_timeout("30")
        .extra_arg("--playlist-end")
        .extra_arg("20")
        .run()
        .expect("Youtube-dlp command ran with errors");

    let videos: Playlist = match output {
        youtube_dl::YoutubeDlOutput::Playlist(playlist) => Some(*playlist),
        _ => None,
    }
    .expect("could not unwrap playlist item");

    let videos: Vec<SingleVideo> = videos
        .entries
        .expect("could not extract videos from Playlist item");

    // println!("{videos:#?}");

    videos
}

#[derive(Debug)]
struct PodInfo {
    id: Guid,
    url: String,
    episode: Option<i32>,
    title: String,
    duration_str: String,
    duration_secs: i64,
    author: String,
    date: String,
    link: String,
    description: String,
}

impl From<rss::Item> for PodInfo {
    fn from()
    
}

impl From<SingleVideo> for PodInfo {
    fn from(video: SingleVideo) -> Self {
        let duration = video
            .duration
            .expect(&format!("could not find a duration for {}", video.id))
            .as_i64()
            .expect("could not convert duration to i64");
        let duration = Duration::seconds(duration);

        let date = NaiveDate::parse_from_str(
            &video
                .upload_date
                .expect(&format!("Could not find an upload_date for {}", video.id)),
            "%Y%m%d",
        )
        .expect(&format!(
            "could not parse video {}'s upload date as str",
            video.id,
        ));

        fn gen_description(description: Option<String>) -> String {
            // "From the creators of the original Fallout games comes Arcanum. A fantasy RPG about the dynamics between technology and magic, and the endless despicability of gnomes.\nSupport the channel at: https://www.patreon.com/mandaloregaming or https://www.paypal.me/MandaloreGaming\nI take video suggestions at mandaloremovies@gmail.com\nTwitter: https://twitter.com/Lord_Mandalore\nBig thanks to @Branzoles for his artistic contributions.\n00:00 - Intro\n00:37 - Issues & Fixes\n01:50 - The World of Arcanum\n05:18 - Game Intro\n06:44 - Visuals\n07:53 - Music & Sound Design\n12:12 - Gameplay Mechanics\n24:42 - Questing & Story\n26:46 - Story (SPOILERS)\n48:42 - Conclusions\n50:04 - Credits\n51:28 - Baah\n\n#Arcanum #ArcanumReview #ArcanumPC #Troika",
            if let Some(description) = description {
                let description: String = description
                    .split("\n")
                    .into_iter()
                    .map(|line| {
                        format!(
                            "<p>{}</p>",
                            html_escape::encode_text_to_string(line, &mut String::new())
                        )
                    })
                    .collect();
                println!("{}", description.clone());
                description
            } else {
                String::new()
            }
        }

        fn gen_duration_str(duration: Duration) -> String {
            let hours = duration.num_hours();
            let minutes = duration.num_minutes() - (&duration.num_hours() * 60);
            let seconds = duration.num_seconds() - (&duration.num_minutes() * 60);

            let time: Vec<String> = vec![hours, minutes, seconds]
                .into_iter()
                .map(|number| format!("{number}"))
                .map(|time_str| match time_str.chars().count() {
                    1 => format!("0{time_str}"),
                    _ => time_str,
                })
                .collect();

            format!("{}:{}:{}", time[0], time[1], time[2])
        }

        PodInfo {
            id: GuidBuilder::default()
                .value(&video.id)
                .permalink(false)
                .build(),
            url: format!("https://0d1f-73-45-179-121.ngrok.io/{}", &video.id),
            episode: None,
            title: video.title,
            duration_str: gen_duration_str(duration),
            duration_secs: duration.num_seconds(),
            author: video
                .uploader
                .expect(&format!("Could not find an uploader for {}", video.id)),
            date: format!("{}", date.format("%a, %d %b %Y 00:00:00 +0000")), // ex: Thu, 06 Oct 2022 15:07:56 +0000
            link: video
                .webpage_url
                .expect(&format!("Could not find a url for {}", video.id)),
            description: gen_description(video.description),
        }
    }
}

impl PodInfo {
    fn update_ep_number(&self, number: i32) -> Self {
        PodInfo {
            guid: self.guid,
            episode: Some(number),
            title: self.title,
            duration_str: self.duration_str,
            duration_secs: self.duration_secs,
            author: self.author,
            date: self.date,
            link: self.link,
            description: self.description,
        }
    }
}

fn build_episode(video: SingleVideo) -> Item {
    let title = &video.title;
    let nv = video.clone();

    let enclosure: Enclosure = EnclosureBuilder::default()
        .mime_type("audio/m4a".to_owned())
        .length(episode.duration_secs.to_string())
        .url(episode.url)
        .build();

    let itunes_metadata: ITunesItemExtension = ITunesItemExtensionBuilder::default()
        .episode(Some("1".to_owned()))
        .author(Some(episode.author))
        .duration(Some(episode.duration_str))
        .block(Some("Yes".to_string()))
        .build();

    // We have to write a whole custom extension just to get <itunes:title>
    // TODO this is magic. Figure out how it works. Like what the heck are the
    // first entries in the BTreeMap about??
    let itunes_title = BTreeMap::from([(
        "itunes_title".to_owned(),
        vec![ExtensionBuilder::default()
            .name("itunes:title".to_owned())
            .value(Some(episode.title.clone()))
            .build()],
    )]);

    let item: Item = ItemBuilder::default()
        .guid(Some(episode.id))
        .pub_date(Some(episode.date))
        .title(Some(episode.title))
        .extensions(BTreeMap::from([("itunes_title".to_owned(), itunes_title)])) // put <itunes:title> in there
        .itunes_ext(Some(itunes_metadata))
        .enclosure(Some(enclosure))
        .link(Some(episode.link))
        .description(Some(episode.description))
        .build();

    item
}

fn read_feed(channel_name: String) -> Result<rss::Channel, rss::Error> {
    let file = format!("{channel_name}.xml");
    let file = File::open(file).map_err(|err| rss::Error::Xml(quick_xml::Error::Io(err)))?;
    let channel = rss::Channel::read_from(BufReader::new(file))?;
    Ok(channel)
}

fn gen_new_feed(current_feed: rss::Channel, new_items: Vec<PodInfo>) -> Vec<PodInfo> {
    let old_items: Vec<PodInfo> = current_feed.into_items().into();

    let old_ids = old_items
        .into_iter()
        .map(|item| item.guid.unwrap().value)
        .collect::<Vec<String>>();

    // FIXME this implementation is quadratic
    let new_episodes: Vec<PodInfo> = new_items
        .into_iter()
        .filter(|item| !old_ids.contains(&item.guid.value))
        .collect();

    // assuming that old_episodes is chronological, most-recent first
    let mut latest_number: i32 = old_items
        .clone()
        .into_iter()
        .next()
        .unwrap()
        .itunes_ext()
        .unwrap()
        .episode()
        .unwrap()
        .parse()
        .unwrap();

    let fixed_new_episodes: Vec<PodInfo> = new_episodes
        .into_iter()
        .rev()
        .map(|episode| {
            latest_number + 1;
            episode.update_ep_number(latest_number + 1)
        })
        .collect::<Vec<PodInfo>>()
        .into_iter()
        .chain();
}

pub async fn gen_feed(channel_id: String) {
    let channel_id = "UCNmv1Cmjm3Hk8Vc9kIgv0AQ".to_owned();
    let recents = get_recent_videos(channel_id);

    let latest: PodInfo = recents.clone().into_iter().nth(0).unwrap().into();

    let ep = build_episode(latest);
    let itunes_metadata = ITunesChannelExtensionBuilder::default()
        .author(Some("Alex Jackson".to_owned()))
        .build();

    let itunes_namespaces = BTreeMap::from([
        (
            "itunes".to_owned(),
            "http://www.itunes.com/dtds/podcast-1.0.dtd".to_owned(),
        ),
        (
            "content".to_owned(),
            "http://purl.org/rss/1.0/modules/content/".to_owned(),
        ),
    ]);

    let channel = ChannelBuilder::default()
        .namespaces(itunes_namespaces)
        .title("Test Title".to_owned())
        .link("http://test.com".to_owned())
        .description("A Test Feed".to_owned())
        .itunes_ext(Some(itunes_metadata))
        .items(vec![ep])
        .build();

    let rss_file = File::create("test.xml").expect("could not create test.xml");

    channel
        .write_to(rss_file)
        .expect("could not write to rss_file");
}
