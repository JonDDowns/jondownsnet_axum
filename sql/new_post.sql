\set post `cat ~/projects/jondownsnet_axum/rust/htmlpages/raw/this_one_mistake.html`
INSERT INTO posts (post_date, post_title, post_body, post_summary, post_thumbnail, post_thumbnail_alt)
VALUES (
	'2024-04-26',
	'This One Mistake Slows Your R Code Down by 200x', 
	:'post', 
	'You might be surprised at how the for loop does.',
	'../assets/images/shuaib-khokhar-unsplash.webp',
	'Speedometer image. Courtesy of Shuaib Khokhar via unsplash.'
);
