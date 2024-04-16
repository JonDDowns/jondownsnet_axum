DROP TABLE IF EXISTS posts;
CREATE TABLE posts(
	post_id SERIAL PRIMARY KEY,
	post_date DATE NOT NULL DEFAULT CURRENT_DATE,
	post_title TEXT NOT NULL,
	post_body TEXT NOT NULL,
	post_summary CHAR(200) NOT NULL,
	post_thumbnail TEXT NOT NULL,
	post_thumbnail_alt TEXT NOT NULL,
	UNIQUE(post_title)
);

\set p1 `cat ./htmlpages/minified/palindromes.html`
\set p2 `cat ./htmlpages/minified/gamesToWin.html`
\set p3 `cat ./htmlpages/minified/getPRISM.html`
\set p4 `cat ./htmlpages/minified/RUserPythonGuide.html`
\set p5 `cat ./htmlpages/minified/rust_r_trigrams.html`
INSERT INTO posts (post_date, post_title, post_body, post_summary, post_thumbnail, post_thumbnail_alt)
VALUES (
		'2021-08-01',
		'Palindromes in R',
		:'p1', 
		'Showcasing unit testing in R with a common coding interview question.', 
		'../assets/images/palindromes.png',
		'Some palindromes with a void background.'
	),
	(
		'2021-08-08',
		'How long will the series last?',
		:'p2',
		'Exploring probability with an Rshiny app.',
		'../assets/images/thomas-park-Nl942-bo_4o-unsplash.jpg',
		'Baseball. Courtesy Thomas Park via Unsplash.'
	),
	(
		'2021-09-22',
		'How to Download a Year of Temperature Data in 144 Lines of R Code',
		:'p3',
		'Demonstrates parallelism and rater manipulation in R.',
		'../assets/images/prismTempsP3.png',
		'Heat map of Washington, USA'
	),
	(
		'2022-01-13',
		'An R User''s Guide to Python',
		:'p4',
		'Compares and contrasts some of the most common data manipulation operations in both R and Python.',
		'../assets/images/dsPrimaDonna.png',
		'Image from The Queen''s Gambit, captioned with a meme. Very funny, trust me.'
	),
	(
		'2024-04-12',
		'Need Performance? Use Rust!',
		:'p5',
		'Implementing and benchmarking cosine string distance in Rust and R.',
		'../assets/images/rust-logo.png',
		'Rust logo. This does not reflect an endorsement of this website by the Rust Foundation.'
	);
