use spacetimedb::{table, reducer, Table, ReducerContext, Identity};
use log::{info, error};
use aws_sdk_s3::{Client as S3Client};
use aws_sdk_s3::config::{Region, Credentials};
use std::env;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

// Helper function to generate a random ID
fn generate_id() -> String {
    let rand_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();
    rand_string
}

// Helper function to get current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// Track table
#[table(name = track, public)]
#[derive(Clone)]
pub struct Track {
    #[primary_key]
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: Option<String>,
    pub year: Option<u16>,
    pub duration_seconds: u32,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub date_added: u64,
}

// Playlist table
#[table(name = playlist, public)]
#[derive(Clone)]
pub struct Playlist {
    #[primary_key]
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub description: Option<String>,
    pub date_created: u64,
    pub is_public: bool,
}

// PlaylistTrack table for many-to-many relationship
#[table(name = playlist_track, public)]
#[derive(Clone)]
pub struct PlaylistTrack {
    #[primary_key]
    pub id: String,
    pub playlist_id: String,
    pub track_id: String,
    pub position: u32,
    pub date_added: u64,
}

// User table
#[table(name = user, public)]
#[derive(Clone)]
pub struct User {
    #[primary_key]
    pub id: String,
    pub username: String,
    pub date_joined: u64,
}

// UserFavorite table
#[table(name = user_favorite, public)]
#[derive(Clone)]
pub struct UserFavorite {
    #[primary_key]
    pub id: String,
    pub user_id: String,
    pub track_id: String,
    pub date_added: u64,
}

// Initialize R2 client
fn get_r2_client() -> Result<S3Client, String> {
    let endpoint = env::var("R2_ENDPOINT").map_err(|_| "R2_ENDPOINT not set".to_string())?;
    let access_key_id = env::var("R2_ACCESS_KEY_ID").map_err(|_| "R2_ACCESS_KEY_ID not set".to_string())?;
    let secret_access_key = env::var("R2_SECRET_ACCESS_KEY").map_err(|_| "R2_SECRET_ACCESS_KEY not set".to_string())?;
    let region = env::var("R2_REGION").unwrap_or_else(|_| "auto".to_string());

    let credentials = Credentials::new(
        access_key_id,
        secret_access_key,
        None,
        None,
        "r2-credentials",
    );

    let config = aws_sdk_s3::config::Builder::new()
        .region(Region::new(region))
        .endpoint_url(endpoint)
        .credentials_provider(credentials)
        .build();

    Ok(S3Client::from_conf(config))
}

#[reducer]
pub fn init(_ctx: &ReducerContext) {
    info!("Initializing music server module");
    // Try to initialize the R2 client
    match get_r2_client() {
        Ok(_) => info!("R2 client initialized successfully"),
        Err(e) => error!("Failed to initialize R2 client: {}", e),
    }
}

#[reducer(client_connected)]
pub fn identity_connected(ctx: &ReducerContext, identity: Identity) {
    info!("Client connected: {}", identity);
    let user_id = identity.to_string();
    
    // Check if user exists
    let user_table = ctx.db.user();
    let users: Vec<User> = user_table.iter()
        .filter(|user| user.id == user_id)
        .collect();
    
    if users.is_empty() {
        let username = format!("user_{}", &user_id[0..8]);
        let user = User {
            id: user_id,
            username,
            date_joined: current_timestamp(),
        };
        
        user_table.insert(user);
    }
}

#[reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext, identity: Identity) {
    info!("Client disconnected: {}", identity);
}

#[reducer]
pub fn add_track(ctx: &ReducerContext, title: String, artist: String, album: String, genre: Option<String>, year: Option<u16>, duration_seconds: u32, file_path: String, file_size_bytes: u64) {
    let track_id = generate_id();
    let track = Track {
        id: track_id.clone(),
        title,
        artist,
        album,
        genre,
        year,
        duration_seconds,
        file_path,
        file_size_bytes,
        date_added: current_timestamp(),
    };
    
    ctx.db.track().insert(track);
}

#[reducer]
pub fn list_tracks(ctx: &ReducerContext) {
    let tracks = ctx.db.track().iter().collect::<Vec<_>>();
    
    // Log the tracks
    for track in tracks {
        info!("Track: {} - {} by {}", track.id, track.title, track.artist);
    }
}

#[reducer]
pub fn search_tracks(ctx: &ReducerContext, query: String) {
    let query = query.to_lowercase();
    let tracks: Vec<Track> = ctx.db.track().iter()
        .filter(|track| {
            track.title.to_lowercase().contains(&query) ||
            track.artist.to_lowercase().contains(&query) ||
            track.album.to_lowercase().contains(&query) ||
            track.genre.as_ref().map_or(false, |g| g.to_lowercase().contains(&query))
        })
        .collect();
    
    // Log the results
    for track in tracks {
        info!("Found track: {} - {} by {}", track.id, track.title, track.artist);
    }
}

#[reducer]
pub fn create_playlist(ctx: &ReducerContext, name: String, description: Option<String>, is_public: bool) {
    let playlist_id = generate_id();
    let user_id = ctx.sender.to_string();
    
    let playlist = Playlist {
        id: playlist_id.clone(),
        name,
        owner_id: user_id,
        description,
        date_created: current_timestamp(),
        is_public,
    };
    
    ctx.db.playlist().insert(playlist);
}

#[reducer]
pub fn add_track_to_playlist(ctx: &ReducerContext, playlist_id: String, track_id: String) {
    // Check if playlist exists
    let playlist_table = ctx.db.playlist();
    let track_table = ctx.db.track();
    let playlist_track_table = ctx.db.playlist_track();
    
    let playlists: Vec<Playlist> = playlist_table.iter()
        .filter(|p| p.id == playlist_id)
        .collect();
    
    if playlists.is_empty() {
        error!("Playlist not found");
        return;
    }
    
    // Check if track exists
    let tracks: Vec<Track> = track_table.iter()
        .filter(|t| t.id == track_id)
        .collect();
    
    if tracks.is_empty() {
        error!("Track not found");
        return;
    }
    
    // Find the highest position
    let playlist_tracks: Vec<PlaylistTrack> = playlist_track_table.iter()
        .filter(|pt| pt.playlist_id == playlist_id)
        .collect();
    
    let max_position = playlist_tracks
        .iter()
        .map(|pt| pt.position)
        .max()
        .unwrap_or(0);
    
    // Add track to playlist
    let playlist_track = PlaylistTrack {
        id: generate_id(),
        playlist_id,
        track_id,
        position: max_position + 1,
        date_added: current_timestamp(),
    };
    
    playlist_track_table.insert(playlist_track);
}

#[reducer]
pub fn get_playlist_tracks(ctx: &ReducerContext, playlist_id: String) {
    let playlist_table = ctx.db.playlist();
    let track_table = ctx.db.track();
    let playlist_track_table = ctx.db.playlist_track();
    
    // Check if playlist exists
    let playlists: Vec<Playlist> = playlist_table.iter()
        .filter(|p| p.id == playlist_id)
        .collect();
    
    if playlists.is_empty() {
        error!("Playlist not found");
        return;
    }
    
    // Get tracks in playlist
    let playlist_tracks: Vec<PlaylistTrack> = playlist_track_table.iter()
        .filter(|pt| pt.playlist_id == playlist_id)
        .collect();
    
    // Create a map of position -> track
    let mut position_track_map = Vec::new();
    
    for pt in playlist_tracks {
        let tracks: Vec<Track> = track_table.iter()
            .filter(|t| t.id == pt.track_id)
            .collect();
            
        if !tracks.is_empty() {
            position_track_map.push((pt.position, tracks[0].clone()));
        }
    }
    
    // Sort by position
    position_track_map.sort_by_key(|(pos, _)| *pos);
    
    // Log the tracks
    for (_, track) in position_track_map {
        info!("Playlist track: {} - {} by {}", track.id, track.title, track.artist);
    }
}

#[reducer]
pub fn add_to_favorites(ctx: &ReducerContext, track_id: String) {
    let user_id = ctx.sender.to_string();
    let track_table = ctx.db.track();
    let favorite_table = ctx.db.user_favorite();
    
    // Check if track exists
    let tracks: Vec<Track> = track_table.iter()
        .filter(|t| t.id == track_id)
        .collect();
    
    if tracks.is_empty() {
        error!("Track not found");
        return;
    }
    
    // Check if already in favorites
    let favorites: Vec<UserFavorite> = favorite_table.iter()
        .filter(|fav| fav.user_id == user_id && fav.track_id == track_id)
        .collect();
    
    // Add to favorites if not already there
    if favorites.is_empty() {
        let favorite = UserFavorite {
            id: generate_id(),
            user_id,
            track_id,
            date_added: current_timestamp(),
        };
        
        favorite_table.insert(favorite);
    }
}

#[reducer]
pub fn remove_from_favorites(ctx: &ReducerContext, track_id: String) {
    let user_id = ctx.sender.to_string();
    let favorite_table = ctx.db.user_favorite();
    
    // Find and remove the favorite
    let favorites: Vec<UserFavorite> = favorite_table.iter()
        .filter(|fav| fav.user_id == user_id && fav.track_id == track_id)
        .collect();
    
    for favorite in favorites {
        favorite_table.delete(favorite);
    }
}

#[reducer]
pub fn get_favorite_tracks(ctx: &ReducerContext) {
    let user_id = ctx.sender.to_string();
    let track_table = ctx.db.track();
    let favorite_table = ctx.db.user_favorite();
    
    // Get favorite tracks
    let favorites: Vec<UserFavorite> = favorite_table.iter()
        .filter(|fav| fav.user_id == user_id)
        .collect();
    
    let mut tracks = Vec::new();
    
    for favorite in favorites {
        let matching_tracks: Vec<Track> = track_table.iter()
            .filter(|t| t.id == favorite.track_id)
            .collect();
            
        if !matching_tracks.is_empty() {
            tracks.push(matching_tracks[0].clone());
        }
    }
    
    // Log the favorite tracks
    for track in tracks {
        info!("Favorite track: {} - {} by {}", track.id, track.title, track.artist);
    }
}

#[reducer]
pub fn delete_track(ctx: &ReducerContext, track_id: String) {
    let track_table = ctx.db.track();
    let playlist_track_table = ctx.db.playlist_track();
    let favorite_table = ctx.db.user_favorite();
    
    // Check if track exists
    let tracks: Vec<Track> = track_table.iter()
        .filter(|t| t.id == track_id)
        .collect();
    
    if tracks.is_empty() {
        error!("Track with ID {} not found", track_id);
        return;
    }
    
    // Delete all playlist entries for this track
    let playlist_tracks: Vec<PlaylistTrack> = playlist_track_table.iter()
        .filter(|pt| pt.track_id == track_id)
        .collect();
    
    for pt in playlist_tracks {
        playlist_track_table.delete(pt);
    }
    
    // Delete all user favorites for this track
    let favorites: Vec<UserFavorite> = favorite_table.iter()
        .filter(|fav| fav.track_id == track_id)
        .collect();
    
    for fav in favorites {
        favorite_table.delete(fav);
    }
    
    // Delete the track itself
    track_table.delete(tracks[0].clone());
    
    // Note: We can't delete the file from R2 here since we can't use async in reducers
    // This would need to be handled separately
}

#[reducer]
pub fn update_track_metadata(ctx: &ReducerContext, track_id: String, title: Option<String>, artist: Option<String>, album: Option<String>, genre: Option<String>, year: Option<u16>) {
    let track_table = ctx.db.track();
    
    // Check if track exists
    let tracks: Vec<Track> = track_table.iter()
        .filter(|t| t.id == track_id)
        .collect();
    
    if tracks.is_empty() {
        error!("Track with ID {} not found", track_id);
        return;
    }
    
    let mut track = tracks[0].clone();
    
    if let Some(title) = title {
        track.title = title;
    }
    
    if let Some(artist) = artist {
        track.artist = artist;
    }
    
    if let Some(album) = album {
        track.album = album;
    }
    
    track.genre = genre;
    track.year = year;
    
    // Delete the old track and insert the updated one
    track_table.delete(tracks[0].clone());
    track_table.insert(track);
}

#[reducer]
pub fn get_stats(ctx: &ReducerContext) {
    // Count tracks
    let track_count = ctx.db.track().count();
    
    // Count playlists
    let playlist_count = ctx.db.playlist().count();
    
    // Count users
    let user_count = ctx.db.user().count();
    
    // Sum duration
    let total_duration: u64 = ctx.db.track()
        .iter()
        .map(|t| t.duration_seconds as u64)
        .sum();
    
    // Sum file size
    let total_size: u64 = ctx.db.track()
        .iter()
        .map(|t| t.file_size_bytes)
        .sum();
    
    // Log the stats
    info!("Track count: {}", track_count);
    info!("Playlist count: {}", playlist_count);
    info!("User count: {}", user_count);
    info!("Total duration: {} seconds", total_duration);
    info!("Total size: {} bytes", total_size);
}
