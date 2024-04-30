\set post `cat ./htmlpages/minified/this_one_mistake.html`
UPDATE posts SET 
	post_body = :'post'
	, post_thumbnail = '../assets/images/shuaib-khokhar-unsplash.webp'
	, post_thumbnail_alt = 'Speedometer image. Courtesy of Shuaib Khokhar via unsplash.'
WHERE post_id = 6;
