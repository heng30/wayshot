use recorder::apply_gain;

fn main() {
    // Example audio data (-6dBFS sine wave)
    let mut audio_data = vec![0.5, -0.5, 0.3, -0.3, 0.1, -0.1];

    println!("Original data: {:?}", audio_data);

    // Apply +0dB gain
    apply_gain(&mut audio_data, 0.0);
    println!("After +0dB gain: {:?}", audio_data);

    // Apply +6dB gain (amplify by 2x)
    apply_gain(&mut audio_data, 6.0);
    println!("After +6dB gain: {:?}", audio_data);

    // Apply -12dB gain (attenuate to approximately 0.25x)
    apply_gain(&mut audio_data, -12.0);
    println!("After -12dB gain: {:?}", audio_data);

    // Apply -120dB gain (essentially mute)
    apply_gain(&mut audio_data, -120.0);
    println!("After -120dB gain: {:?}", audio_data);
}
