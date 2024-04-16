\set post `cat ~/projects/jondownsnet_axum/raw_html/rust_r_trigrams.html`
INSERT INTO posts (post_date, post_title, post_body, post_summary)
VALUES (
	'2024-04-11',
	'Need Performance? Use Rust!', 
	:'post', 
	'Comparing cosine string distance performance in R and Rust.'
);
